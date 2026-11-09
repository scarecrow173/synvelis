---
id: "ADR-034Uqu6fjJkQjiqAsPYYA1"
type: "adr"
status: "accepted"
title: "PC-C3 ControlTrace — stage slot evidence model・freshness 評価境界・probe 状態語彙"
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
    - "docs/specs/pc-c3-control-trace-projection.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c1-recording-transaction-prepare-apply-finalize.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C3 ControlTrace — stage slot evidence model・freshness 評価境界・probe 状態語彙

## Status

accepted（review-protocol によるレビュー済み・blocker なし）

## Context

Issue #100 は Configure→Prepare→Install/Applied→Dispatch→PluginReport→Probe の証拠を一貫した typed model で投影することを要求する。既存要素の制約:

- `ControlEvaluation`/`EvalPlan`（`control.rs`）は評価 graph の compile 証拠を持つが、dispatch 以降の stage 証拠を持たない。
- `ParamDispatch`/`ParamDispatchLedger`/`DispatchStage`/`NodeReport`（`synvelis-player`・Ruviform bridge）は dispatch 経路の証拠を持つが project 層に投影されない。本 slice では型の意味論を合わせる参照のみで変更しない。
- `ControlObservation`/`ObservationState`（`synvelis-workspace`）は plugin_reported/probe の 2 channel を持つが、本 slice では変更しない。
- `routing_build.rs` は `RoutingBuildKey`（generation-scoped）・`RoutingFault`・`RoutingActivationState`・`ExecutionEquivalence` で last-good semantics を提供する — 再利用すべき先例。
- `probe.rs` は `ProbeStatusReceipt`/`ObservationGap`/`ProbeResolveState`/`RuntimeProbeId`/`ClockContextRef`/`SemanticProbeBinding` を提供する。

決定すべき論点: (a) stage 証拠をどう型付けするか、(b) freshness をどの境界で評価するか、(c) probe の missing/lost/disabled/Measured Silence をどう区別するか、(d) 失敗 plan の active 投影をどう表すか。

## Considered Options

### Option 1: 単一 `TraceEvent` 列 + 消費側で stage 解釈

イベント列をそのまま保持し stage 判定を consumer に委ねる。→ 却下: 欠落 stage の Unknown 維持・generation 相関・freshness 分類が consumer ごとに重複し、誤 Live を構造的に防げない。

### Option 2: 7 slot の独立 `TraceSlot<T>` evidence model（採用）

各 stage を独立した optional slot とし、欠落理由（`Missing`/`Unsupported`/`Dropped`/`Expired`）を typed に保持。generation/epoch/position の相関は `ControlTraceKey` が担う。freshness は観測 channel（plugin_reported / observed）に対して評価コンテキストと比較して行う。

### Option 3: `ControlObservation` 拡張で workspace に寄せる

workspace の観測型を拡張。→ 却下: Issue の編集可範囲外（workspace 変更禁止）であり、project 層の projection として分離する方が境界が明確。

## Decision

**Option 2 を採用。**

1. **stage slot evidence model**: `ControlTrace` は 7 つの `TraceSlot<T>` field を持つ。`TraceSlot` は `Present(T)` / `Missing` / `Unsupported` / `Dropped` / `Expired`。`Present` 以外は「証拠が無い/使えない」ことを typed に表し、合成しない。
2. **freshness 評価境界**: `ObservationFreshness` は `plugin_reported` と `observed` の観測 channel に対してのみ評価する（configured/prepared 等は dispatch 証拠であり Live 判定対象外）。評価は `FreshnessContext`（現在の revision/plan/graph/transport/epoch/position/now/max_age）と証拠 stamp の比較で行い、`StampProvenance::Estimated` のみでは `Current` にしない。
3. **probe 状態語彙**: `ProbeChannelState` = `Missing`（binding なし）/ `Disabled`（bind あるが観測 off）/ `Lost{count}`（gap 検出）/ `Measuring{stamp, value}`。`MeasuredSilence` は `MeasuredValue::Silence` として `Measuring` と同じ slot 内で区別し、「probe が無い/壊れた」と「無音を測定した」を混同しない。
4. **plan activation**: `PlanActivation` = `Active{plan_generation, ..}` / `Failed{plan_generation, fault}` / `Pending{plan_generation}`。失敗 plan は `Failed` として保持し、active slot は旧 plan を指し続ける（`routing_build.rs` の `RoutingActivationState`/`RoutingFault`/`ExecutionEquivalence` の意味論に整合）。

## Consequences

- `synvelis-player`/`synvelis-workspace`/Ruviform は変更しない。型の意味論は `DispatchStage`/`ParamDispatchLedger`/`ControlObservation` と整合させるが、変換は後続 integration slice の責務。
- `TraceSlot` の `Dropped`/`Expired` は「証拠があったが相関前に失われた/失効した」ことを表し、`Missing`（最初から無い）と区別する。
- freshness の `Estimated` は「stamp が estimated provenance」— 値は保持するが `Current` とはしない。
- 2 Inspector 二重起動防止は本 slice の対象外。`ProbeChannelState::Disabled`/`Missing` の区別が workspace 接続時の判断材料になることを文書に記録する。
