---
id: "TSPEC-034UqHaJf2vZgpPzjlqqi7"
type: "test-spec"
status: "approved"
title: "PC-C3 ControlTrace E2E — stage 相関・freshness・probe 状態・plan activation"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/pc-c3-control-trace-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c3-control-trace-projection.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C3 ControlTrace 検証仕様

fixture test `crates/synvelis-project/tests/trace_c3.rs` が担保する契約。実 plugin/OS/RT 観測は対象外 — 全て合成 fixture。

## Feature: stage slot 独立性と欠落 Unknown 維持（TRACE-C3-01）

- Rule: 各 slot は独立。`Present` 以外は `Missing`/`Unsupported`/`Dropped`/`Expired` を保持し、他 stage の証拠から推論・合成しない。
- Rule: receipt 相関は `ControlTraceKey` の generation 群で行う。`ScheduledReceipt`/`AppliedStageReceipt` は status が一致する receipt のみ構築可能（fail-closed）。
- Example: configured+prepared のみ Present の trace で applied/dispatched/plugin_reported/observed が `Missing` のまま → `freshness()` は `Unavailable{NoEvidence}` 系。scheduled に `Applied` status receipt を入れようとしても `new` が `None`。

## Feature: freshness 分類（TRACE-C3-02）

- Rule: key↔ctx の `project_revision`/`control_plan_generation`/`graph_generation`/`transport_generation`/`clock_epoch`/`loop_occurrence`/`evaluation_position` 不一致は対応する `StaleReason` で `Stale`。誤 `Current` なし。
- Rule: stamp `provenance == Estimated` は `Estimated`（Current にしない）。`now > stamp.time + max_age` は `Stale{Age}`。`now < stamp.time`・deadline 算出 overflow は `Unavailable{ClockUntrusted}`。
- Example: Seek（transport_generation bump）後の ctx で旧 trace を評価 → `Stale{Transport}`。DeviceRestart（clock_epoch bump）→ `Stale{ClockEpoch}`。loop wrap（loop_occurrence bump）→ `Stale{Loop}`。

## Feature: probe 状態と Measured Silence（TRACE-C3-03）

- Rule: `ProbeObservation::Missing`/`Disabled`/`Lost{count}`/`Measuring{stamp, value}` を区別。`MeasuredValue::Silence` は `Measuring` の value であり probe 欠落ではない。
- Rule: `Lost{count}` → `DataLoss{count}`。`Missing` → `Unavailable{ProbeMissing}`。`Disabled` → `Unavailable{ProbeDisabled}`。
- Note: 2 Inspector 二重起動防止は workspace 接続時の責務 — 本 test では検証しない（文書記録のみ）。

## Feature: dispatch 意味論分離と失敗 plan 投影（TRACE-C3-04）

- Rule: `Dispatched` evidence は投影（送信済み）であり plugin 内部処理・出力実測を意味しない — `plugin_reported`/`observed` slot とは独立。
- Rule: `PlanActivationState` は `active`（last-good 継続）と `last_attempt`（失敗・非 active）を分離。失敗 plan の `plan_generation` が `active` に現れない。
- Rule: `is_coherent()` は slot 内 evidence の generation が key と一致するかを検査し、不一致を `false` で報告。
- Example: plan B publish 失敗後、`active` は plan A のまま、`last_attempt` が plan B + `RoutingFault` を保持。

## Guarantees

- 全判定は決定論的・read-only・非 RT。project state を変更しない。
- 欠落・失敗・stale を成功・Live と誤認しない。

## Non-goals

- TRACE-C3-05・実 plugin/OS/RT 観測・workspace `ControlSession` 接続・#101/#103/#114 統合。

## Risk

- bridge 型の semantic mirror drift — integration slice で変換時に再検証。
