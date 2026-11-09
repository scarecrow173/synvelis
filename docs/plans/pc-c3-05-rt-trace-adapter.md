---
id: "PLAN-qCDk2ELwZ5XT36rSkQsD8F"
type: "plan"
status: "approved"
title: "PC-C3-05 RT trace adapter 実装計画"
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
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/designs/pc-c3-05-rt-trace-adapter-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c3-05-rt-trace-adapter.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C3-05 RT trace adapter 実装計画

## Context

TRACE-C3-05 slice: player 層で dispatch ledger 実イベントを `ControlTrace` に写像する adapter。前提: project 層 trace 型（PR #206）・`ParamDispatch`/`ParamDispatchLedger`（PC-B）実装済み。

## Scope

- In: `src/trace.rs` 新設、`src/dispatch.rs` 最小追加、`src/lib.rs` export、`Cargo.toml` dev-dep、tests 2 本、docs。
- Out: workspace/driver 接続・control() 再公開・実 plugin report・他 crate（spec Out of Scope 参照）。

## Implementation Steps

1. **`src/dispatch.rs` 拡張**
   - `DispatchCycle { observations, receipts }` 追加。`observe` 本体を `observe_cycle` へ移し、receipt（`NodeReport::Receipt` の `AppliedReceipt` clone）を収集して返す。`observe` は `observe_cycle(...).observations` を返す wrapper に再構成（drain 順序・scheduler_gen ゲート・call-window report 適用順は不変）。
   - `record_reported(target, value) -> bool`（`in_range` gate + `ledger.record_reported`）、`mark_reporting_unavailable()` passthrough 追加。
2. **`src/trace.rs` 実装**
   - `TraceAdapterConfig`（`history_capacity=256`, `max_targets=1024`, `plan_gen_window=64`）、`TraceContext`、`TraceMetrics`、`TraceEvent`/`TraceEventKind`（`#[non_exhaustive]`）、`DispatchTraceAdapter`。
   - 所有権 API: `new(dispatch, frame_base, config, ctx)`、`bind`/`dispatch`（`ParamDispatch` へ委譲 + 成功時に `prepared` slot・plan_gens 記録）、`observe_cycle(span, call_start, call_frames, ctx)`（`dispatch.observe_cycle` → receipt/observation 写像）、`record_configured`/`record_probe`、`record_reported`（ledger 経由 → 次回 observe で反映）、query（`trace`/`traces`/`metrics`/`history`/`history_len`/`is_truncated`）。
   - 写像規則は design「写像規則」表に従う。`DispatchStage`/`DropReason` → project `DispatchEvidence`/`DispatchDropReason` の mirror 関数を adapter 内に集約。
3. **`src/lib.rs`**: `pub mod trace;` + `DispatchTraceAdapter`/`TraceAdapterConfig`/`TraceContext`/`TraceMetrics`/`TraceEvent`/`TraceEventKind`/`DispatchCycle` export。
4. **`Cargo.toml`**: `[dev-dependencies] stats_alloc = "0.1"`（synvelis-project と同版）。
5. **Tests**
   - `tests/trace_c3_05.rs`: pc_b.rs の fake plugin fixture を再利用した in-process dispatch で stage 記入・receipt 相関（`is_coherent`）・bounded history/lost・gen window Expired・PlanReplaced/Dropped・record_configured/record_probe。
   - `tests/trace_c3_05_rt.rs`: `rt_` prefix 単一 test、warm-up 後 `stats_alloc::Region` で `record_probe`+`record_configured`+`trace()`+`metrics()` の 0 alloc/0 dealloc を assert。
6. **docs/impl/ir 記録** + index 再生成。

## Risks and Trade-offs

- ledger revision ↔ dispatch ordinal の 1:1 前提は upstream 規約依存 — ADR/design に明記済み。
- receipt は node 単位 — plan_generation 一致 target のみ帰属（誤帰属しない）。
- `plugin_reported` stamp は Estimated — 実 report channel 実装時に更新余地。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt`（canonical）。
- 受入: bounded history + lost 表面化、RT hot path no-alloc 実測、f64/clock/latency/lost metrics、dispatched/plugin_reported/observed slot が実 ledger イベントで Present + receipt 相関。

## References

- spec: `docs/specs/pc-c3-05-rt-trace-adapter.md`
- adr: `docs/adr/pc-c3-05-trace-adapter-drain-history.md`
- design: `docs/designs/pc-c3-05-rt-trace-adapter-design.md`
- task: `docs/tasks/pc-c3-task2-rt-trace-adapter.md`
- test-spec: [pc-c3-05-rt-trace-adapter](../test-specs/pc-c3-05-rt-trace-adapter.md)
