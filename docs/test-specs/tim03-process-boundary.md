---
id: "TSPEC-034UyEDDz7mTejYlyhg4gF"
type: "test-spec"
status: "approved"
title: "TIM-03 process boundary test spec — TIM-03-PB01..NN"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/tim03-task1-process-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
  verified-by: []
  references:
    - "docs/tasks/tim03-task1-process-boundary.md"
  defers: []
  deferred-by: []
---

# TIM-03 process boundary test spec — TIM-03-PB01..NN

## Purpose

Issue #127 process boundary slice の受入条件を headless mock で
検証する test の規範。evaluator（policy oracle）・実
`TransportScheduler`・実 `PluginNode`/`GraphRuntime` の 3 層で
同一契約を固定する。

## Feature

- `ProcessCapability::evaluate`（`tim03_pb_*`）。
- `TransportScheduler::run_callback` の span/event/receipt/fault
  （`tim03_sched_*`）。
- `PluginNode` 物理 `process` call（`tim03_node_*`）。
- RT hot path no-alloc（`tim03_process_boundary_rt.rs`）。

## Rules

- **PB-01 window/boundary 行列**: 100/128/256f × offset
  0/1/middle/last で Exact plan・連続 segment・legal chunk のみ。
  offset 0 は head apply（cut なし）。
- **PB-02 min/max legality**: `min_frames=128`/`max=512` で
  offset 100 の 256f window は `[0,100)` illegal →
  `ConsentRequired`（RT 無同意）/`OfflineExact`（offline）/
  `DeferralExceeded`（deferral 不足）/consent 付きは 100→128 に
  defer し `BoundaryDegradation` 記録・`DeliveryPolicy::Degraded`。
- **PB-03 同時境界 coalesce**: 同一 offset の LoopWrap+Seek+
  PlanInstall は 1 boundary に収束（kind は決定的）。
- **PB-04 precision gate**: `Unsupported` → typed `Unsupported`。
  `OfflineExact` は certified exact-class 必須
  （`Unknown`/`BlockBoundaryOnly`/`DegradedWithConsent` →
  `OfflineUncertified`）。`Unknown` capability は構造配送のみで
  precision 昇格なし。`Verified` in-block →
  `ExactWithinVerifiedCapability`。
- **PB-05 invalid request**: window=0・offset>window・
  `max_frames==0` degenerate contract は typed error（panic なし）。
- **PB-06 degraded coalescing**: deferred boundary が後続 requested
  offset を applied frame に coalesce。loop wrap は deferral scan
  を bound しない（upstream `find_legal_boundary` 準拠）。
- **PB-07 scheduler Exact**: illegal apply boundary →
  `UnsupportedExactTransport` reject + `UnrepresentableBoundary`
  fault + last-good 継続。illegal wrap → undeliverable span +
  `UnrepresentableLoopWrap` fault + wrap 継続。
- **PB-08 scheduler Degraded**: consented deferral →
  `DeferredApply{requested,applied}` receipt、全 span deliverable、
  call は legal のみ。
- **PB-09 同時境界+event**: wrap+seek+plan install が同一 frame に
  収束 → span は 1 分割、position は wrap collapse（600 in
  loop[0,200] → pos 0）、event は install 後 span で exactly once。
- **PB-10 event/stale generation**: absolute event は position
  到達で exactly once/occurrence、`TransportGeneration` bound
  event は seek 後 stale → `StaleEventGeneration` fault +
  誤配送 0。`PlanGeneration` 不一致も同様。
- **PB-11 node 物理 call**: 全 recorded `process` call が
  `min<=frames<=max`（0-length call なし）。degraded attach では
  deferred boundary が legal call 分割として観測される。
- **PB-12 node event offset**: param event は process-start-
  relative `frame_offset` + call-start `transport.position` で
  verified frame base 配送（abs = position + offset）。
- **PB-13 RT no-alloc**: `stats_alloc` Region 内で 64 callback
  （wrap boundary + in-loop event 含む）allocations=0/
  deallocations=0。

## Examples

- 256f window・offset 100・contract(128,512): RT 無同意 →
  `ConsentRequired{[0]}`; offline → `OfflineExact{0,100}`;
  consent(64) → plan boundaries=[128], degradations=[100→128],
  segments=[(0,128),(128,256)]。
- `TransportScheduler` Exact + Seek@100: receipt Rejected/
  `UnsupportedExactTransport`、span [0,256) deliverable 1 本。
- `TransportScheduler` Degraded(deferral=64) + Seek@100:
  spans [0,128),[128,256)、receipt degradation
  `DeferredApply{100→128}`。
- loop[0,128)+param7@pos64: 256f callback → calls 128+128、
  event は各 occurrence で offset 64 に 1 回ずつ配送。

## Guarantees

- oracle plan の全 `CallPartition` chunk は `legal_call_len` を
  満たす（`SegmentPlan.partition` は構成時に legal のみ）。
- `DegradedConsent` は `Default` 非実装・`grant` 経由のみ。
- typed error variant は spec の全失敗系を網羅
  （`InvalidRequest`/`Unsupported`/`ConsentRequired`/
  `OfflineExact`/`OfflineUncertified`/`DeferralExceeded`）。
- stale generation の param event は plugin に 1 件も届かない
  （`all_params` に param_id が存在しない）。

## Non-goals

- 実 CLAP/VST3/OS matrix（NotRun — 別 gate）。
- oracle の composition root/attach 配線。
- `TimingPrecision` 認定 provenance の定義。
- boundary offset の Tempo/Loop 由来自動導出。

## Risk

- oracle と upstream scheduler の semantics 乖離 → deferral
  `next_bound`・coalescing・wrap collapse を test で upstream
  実測値に固定済み。残余 risk は oracle 未配線のため実害なし。
