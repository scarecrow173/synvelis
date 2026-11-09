---
id: "TSPEC-10rDlBIZV1BQa6OmERMdbJ"
type: "test-spec"
status: "approved"
title: "PC-C3-05 RT trace adapter テスト仕様"
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
    - "docs/designs/pc-c3-05-rt-trace-adapter-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C3-05 RT trace adapter テスト仕様

## Context

fake/in-process dispatch（実 `PluginNode` + `GraphRuntime` 駆動）で adapter の全経路を検証する。実 plugin report・実 OS/RT は対象外。

## Test Matrix

| ID | 対象 | 検証内容 | 期待 |
| --- | --- | --- | --- |
| TRC-T1 | `dispatch` → `observe_cycle` | `prepared` slot に `PreparedEvidence{plan_generation}`、key.control_plan_generation が plan.generation と一致、`dispatched` slot に `Scheduled`→`Dispatched`/`Dropped` の `DispatchEvidence` | `Present` + generation 一致 |
| TRC-T2 | `observe_cycle`（実 node 駆動） | `NodeReport::Receipt` → `ReceiptEvidence` → `scheduled`/`applied` slot。`plan_generation == key.control_plan_generation` の target のみ帰属 | `Present` + `is_coherent()` |
| TRC-T3 | `record_reported` + observe | `DispatchStage::Reported` → `dispatched`=Reported + `plugin_reported`=Present（`Estimated` stamp） | 両 slot `Present` |
| TRC-T4 | `mark_reporting_unavailable` + observe | `DispatchStage::Unavailable` → `dispatched`=Unavailable | `Present(Unavailable)` |
| TRC-T5 | `record_configured`/`record_probe` | `configured`/`observed` slot に外部証拠をそのまま格納（Missing/Disabled/Lost/Measuring 保持） | `Present` |
| TRC-T6 | bounded history | `history_capacity` cap 超過 → ring 溢れ・`metrics` lost counter・`is_truncated()` | 有界 + lost 表面化 |
| TRC-T7 | `max_targets` | target 数 cap 超過 → 最終更新最古を退避・`metrics` lost 計上 | 有界 + lost 表面化 |
| TRC-T8 | `plan_gen_window` | window 外 revision の observation → `dispatched` slot `Expired`・metrics 計上 | `Expired`（合成しない） |
| TRC-T9 | PlanReplaced | 2 回目 dispatch で旧 plan の Scheduled entry → `Dropped{PlanReplaced}` → `DispatchDropReason::PlanReplaced` | `Dropped` evidence（成功にしない） |
| TRC-T10 | metrics | latency（f64 秒）・clock stamp・events/lost counters が実測値と一致 | 実値検証 |
| TRC-T11 | RT hot path | warm-up 後 `stats_alloc::Region` で `record_probe`/`record_configured`/`trace()`/`metrics()` の 0 alloc/0 dealloc | `rt_` test で実測 |

## Fixture

`tests/trace_c3_05.rs` は `pc_b.rs` の `host_plugin`/`GraphRuntime` 駆動 fixture を再利用（in-process fake plugin、決定的）。`tests/trace_c3_05_rt.rs` は adapter を構築し hot path のみ計測。

## NotRun

- 実 plugin→host param report channel（upstream 未実装）— `Reported`/`Unavailable` は fixture 経路のみ。
- 実 OS/RT 観測・workspace `ControlSession` 接続 — 後続 slice。
