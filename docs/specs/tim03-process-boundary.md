---
id: "SPEC-034UyE1s4ix4GwnKIOuV2N"
type: "spec"
status: "approved"
title: "TIM-03 plugin process boundary — capability/degraded policy（headless mock slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/ruviform/issues/307"
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
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  defers: []
  deferred-by: []
---

# TIM-03 plugin process boundary — capability/degraded policy（headless mock slice）

## What Should Be Built

Issue #127（TIM-03 Plugin Process 境界・Capability/Degraded Policy）の
うち、`synvelis-player` crate 内で完結する process boundary slice を
実装する。Ruviform が所有する generic な `BlockContract`/`CallPartition`/
transport-span scheduler/`PluginNode` の上に、Synvelis consumer 側の
precision/degraded/offline ポリシー層 `process_boundary` module を新設し、
headless mock plugin で検証する。

- **`ProcessCapability`**: 1 plugin instance の activated process
  contract（`min_frames`/`max_frames`/in-block transport support/
  certified `TimingPrecision`）を保持し、`ProcessBoundaryRequest` を
  評価して `ProcessBoundaryPlan` または typed `ProcessBoundaryError`
  を返す非 RT preflight oracle。
- **`TimingPrecision`**: Issue 語彙どおり
  `ExactWithinVerifiedCapability`/`FrameAccurateEvents`/
  `BlockBoundaryOnly`/`DegradedWithConsent`/`Unsupported`/`Unknown`。
- **`DegradedConsent`**: `Default` を持たず `DegradedConsent::grant` のみで
  生成される明示同意 token。`max_deferral` を持ち、RT degraded 選択の
  必須条件。
- **`ProcessBoundaryRequest`**: callback window 長・discontinuity
  offsets（Loop wrap/Seek/Plan install 等）・realtime vs offline-exact
  mode・optional consent・`ScheduleBudget`。
- **`ProcessBoundaryPlan`**: 選択 `DeliveryPolicy`、達成 precision、
  legal `SegmentPlan`/`CallPartition`、記録済み `BoundaryDegradation`。

## Why Now

Issue #127 は plugin 実行精度の consumer 側契約を要求する。PC-C4
（#221 merged）で parameter binding/frame base/capability gate の
consumer 側基盤が main 入り済みであり、本 slice はその直上に
process 境界ポリシーを接続する。境界を誤ると 100f のような
`min_frames` 未満の物理 process call が発生し得るため、plan 段階で
legal partition のみを保証し、未対応 capability は typed
`Unsupported`、RT degraded は明示同意、Offline Exact は fail-closed
error とする契約が必要。

## Users and Value

- Synvelis playback/engine 統合層: process 開始前に境界の合法性と
  精度クラスを preflight でき、違法な物理呼出を未然に防げる。
- render/offline 経路: Exact 要求が certified capability を持たない
  plugin に対して fail-closed で拒否され、品質低下が黙って起きない。
- 検証者: headless mock で 100/128/256f window・boundary 位置・
  min/max process length・generation 安全性を決定的に検証できる。

## Scope

### In Scope

- `crates/synvelis-player/src/process_boundary.rs` の新設と `lib.rs`
  export（`TimingPrecision`/`ProcessCapability`/`DegradedConsent`/
  `ProcessBoundaryRequest`/`ProcessBoundaryPlan`/`ProcessBoundaryError`/
  boundary・segment 型）。
- `ProcessCapability::evaluate` による境界 legality 判定:
  - 全 segment が `BlockContract::legal_span_len` で合法 partition
    可能 → `DeliveryPolicy::Exact`。
  - RT で違法 boundary → consent なしは `ConsentRequired`、consent
    ありは `min(consent.max_deferral, budget.max_deferral)` 内で
    次の合法 boundary へ defer し `BoundaryDegradation` を記録。
    範囲内に合法 boundary がなければ `DeferralExceeded`。
  - `OfflineExact` で違法 → `OfflineExact` error（consent でも救済
    しない）。`OfflineExact` で合法だが certified exact-class でない
    → `OfflineUncertified`。
  - `Unsupported` capability → typed `Unsupported`（fake success
    なし）。
- `crates/synvelis-player/tests/tim03_process_boundary.rs`:
  evaluator・実 `TransportScheduler` span・実 `PluginNode`/`GraphRuntime`
  物理 call の 3 層検証。
- `crates/synvelis-player/tests/tim03_process_boundary_rt.rs`:
  `stats_alloc` による RT hot path no-alloc/no-dealloc 実測。
- 本 doc chain（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- 実 plugin CLAP/VST3・OS Backend×Rate×Block matrix — **NotRun**
  （実機 gate、Ruviform #196/#253 側）。
- AudioLens f64 実測 — #294 の別 Gate（f64 native PCM 非退行は
  型レベルで維持）。
- 実 GUI — #114/#176 の別 Gate。
- Ruviform 側の変更（`BlockContract`/scheduler/`PluginNode` は
  消費のみ）。
- `synvelis-app`/`synvelis-workspace`/`synvelis-recording`/
  `synvelis-project`/`synvelis-library`・gateway/render/theme/poc・
  `.github/`・`mise.toml`。
- `ProcessCapability::evaluate` の RT 経路組込み（本 slice は
  non-RT oracle + mock 検証まで。composition root への配線は後続）。

## Requirements

- 100/128/256f window・boundary offset 0/1/100/last frame で決定的な
  plan が得られ、全物理 call が `min_frames <= frames <= max_frames`
  を満たす（不正 Process Length 0 を fixture で検証）。
- Loop/Seek/Plan install 同時境界は決定的に 1 boundary に収束し、
  Param event は process-start-relative offset で verified frame
  base 経由で配送される。
- Capability 未対応は typed `Unsupported`。RT degraded は
  `DegradedConsent` 必須。Offline Exact は fail-closed error。
- Plugin state/clock generation mapping 失効時の誤 Param 配送 0
  （stale `TransportGeneration`/`PlanGeneration` は fault +
  非配送）。
- RT パス no alloc/no blocking（`test-rt` lane で `stats_alloc`
  実測）。

## Acceptance Criteria

- [x] 100/128/256f・boundary=0/1/100/last・Tempo+Loop+seek+Param
  event 同時・min/max frames で不正 Process Length 0 —
  `tim03_pb_*`/`tim03_sched_*`/`tim03_node_*` で検証。
- [x] Capability 未対応 typed `Unsupported`・RT Degraded 明示同意・
  Offline Exact エラー・state/Clock mapping 失効時誤 Param 配送 0 —
  `tim03_pb_precision_gates`/`tim03_pb_min_max_illegal_segment_policy`/
  `tim03_node_stale_transport_generation_zero_delivery`。
- [x] headless mock で検証・RT noalloc/no blocking（`test-rt`）—
  `tim03_process_boundary_rt.rs` の `stats_alloc` Region で
  allocations=0/deallocations=0。

## Deferred Design Concerns

- 実 CLAP/VST3 plugin・OS backend×rate×block matrix —
  Ruviform #196/#253 owner の実機 gate。本 slice の型契約は
  mock でのみ検証済み。
- `ProcessCapability::evaluate` を composition root/attach 経路へ
  配線し、plan 結果を `DeliveryPolicy`/`ScheduleBudget` へ反映する
  統合 — 後続 slice。
- AudioLens f64 実測・実 GUI 検証 — それぞれ #294/#114/#176 の
  別 Gate。

## Sources and Relations

- Issue #127 task spec（本 slice の受入条件の出典）。
- PC-C4 spec/design（#221 merged）: `PluginBindingSession`/
  `AttachClockContext`/`check_frame_base` の consumer 側基盤。
- Ruviform `ruviform-media-core` `transport_schedule.rs`:
  `BlockContract`/`CallPartition`/`DeliveryPolicy`/`ScheduleBudget`/
  `TransportScheduler`/`SpanSink`/`event_engine_frame` — 本 module が
  消費する generic 機構。
- Ruviform `ruviform-audio-plugin-bridge` `PluginNode`/`attach`:
  physical call 経路と `NodeReport`/`PluginNodeControl`。
