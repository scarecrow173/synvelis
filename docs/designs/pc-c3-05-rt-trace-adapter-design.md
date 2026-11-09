---
id: "DESIGN-o5U9K2sOik9XcS81AkO1Qj"
type: "design"
status: "approved"
title: "PC-C3-05 RT trace adapter 設計 — DispatchTraceAdapter・写像・bounded history・metrics"
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
    - "docs/adr/pc-c3-05-trace-adapter-drain-history.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c3-control-trace-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C3-05 RT trace adapter 設計

## Context

`crates/synvelis-player` に `ParamDispatch`/`ParamDispatchLedger`/`NodeReport`/`DispatchStage` の実イベントを `synvelis_project::trace::ControlTrace` へ写像する adapter を追加する。`synvelis-project` は依存方向上位（player → project）で `trace.rs` は read-only 消費のみ。bridge ledger は非 RT・plan generation 非保持。

## Scope

- In: `src/trace.rs`（新規）、`src/dispatch.rs`（`observe_cycle`/`DispatchCycle`/`record_reported`/`mark_reporting_unavailable` 追加）、`src/lib.rs` export、`Cargo.toml` dev-dep `stats_alloc`、tests 2 本、docs。
- Out: workspace/driver 接続・`control()` 再公開・実 plugin report・他 crate 変更（spec 参照）。

## Components and Boundaries

```
src/trace.rs
├── DispatchTraceAdapter     — ParamDispatch 所有・drain 単一経路・trace/metrics/history
│     dispatch: ParamDispatch          （非公開 — drain は observe_cycle のみ）
│     targets: BTreeMap<ParameterTargetId, TargetTrace>   （max_targets で有界）
│     history: VecDeque<TraceEvent>                      （history_capacity ring）
│     plan_gens: Vec<PlanGeneration> + base revision      （plan_gen_window 相対 window）
│     gen_ctx: Option<(TransportGeneration, ClockEpochId)>（receipt 優先の live gens）
│     ctx: TraceContext               （project_id/revision/graph_generation）
│     metrics: TraceMetrics           （counter 蓄積）
├── TraceAdapterConfig       — history_capacity / max_targets / plan_gen_window
├── TraceContext             — { project_id, project_revision, graph_generation }
├── TraceMetrics             — { events, lost 内訳, latency, clock } スナップショット
├── TraceEvent / TraceEventKind — Copy の history record
└── TargetTrace（private）    — { trace: ControlTrace, last_obs: Option<(u64, DispatchStage)>, last_seq }
```

## dispatch.rs 追加（最小）

- `pub struct DispatchCycle { pub observations: Vec<(ParameterTargetId, ParamValueObservation)>, pub receipts: Vec<AppliedReceipt> }` — `observe` の内部結果。`observe` は `observe_cycle(...).observations` を返す薄い wrapper に再構成（drain/project/report 適用の順序・scheduler_gen ゲートは不変）。
- `observe_cycle(&mut self, span, call_start, call_frames) -> DispatchCycle` — 既存 observe 本体 + receipt 収集。
- `record_reported(&mut self, target: ParameterTargetId, value: ParamValue) -> bool` — `in_range` gate + `ledger.record_reported` passthrough。
- `mark_reporting_unavailable(&mut self)` — `ledger.mark_reporting_unavailable` passthrough。

## 写像規則

| 入力 | 出力 slot | 規則 |
| --- | --- | --- |
| `dispatch()` 成功 | `prepared` | `PreparedEvidence{plan_generation}`、key.control_plan_generation 更新、対象 target の position に最終 write の `TimedParamEvent.position` を evaluation_position として記録 |
| `DispatchStage::Scheduled` | `dispatched` | `DispatchEvidence::Scheduled{value, position, plan_generation=rev→gen}` |
| `DispatchStage::Dispatched{call_start,frame_offset}` | `dispatched` | `DispatchEvidence::Dispatched{…, call_start: EngineFrame::new(epoch, call_start), frame_offset}` + latency 計測 |
| `DispatchStage::Dropped{reason}` | `dispatched` | `DispatchEvidence::Dropped{…, reason: DropReason→DispatchDropReason 1:1 mirror}` |
| `DispatchStage::Reported` | `dispatched` + `plugin_reported` | `DispatchEvidence::Reported{value, position}` + `PluginReportedEvidence{value, stamp: estimated(eval_position)}` |
| `DispatchStage::Unavailable` | `dispatched` | `DispatchEvidence::Unavailable{position}` |
| `NodeReport::Receipt` | `scheduled`/`applied` | status Requested/Pending→scheduled、Applied→applied（status class 別に最新）。`plan_generation == key.control_plan_generation` の target のみ。`Rejected` は埋めない |
| `record_configured` | `configured` | `ConfiguredEvidence` をそのまま Present |
| `record_probe` | `observed` | `ProbeObservation` をそのまま Present（Missing/Disabled/Lost/Measuring 全て保持） |

- `value: Option<ParamValue>` が `Scheduled`/`Dispatched`/`Dropped`/`Reported` で `None` の場合は写像不能 → slot 据置（合成しない）。
- `revision → plan_generation` が window 外 → `dispatched` slot を `TraceSlot::Expired` に更新（metrics.expired 計上）。key.control_plan_generation は据置。
- key の caller field（project_id/revision/graph_generation）は新規証拠記録時に `self.ctx`（直近 observe の ctx、初期は constructor ctx）で更新。position/loop/transport/clock は証拠種別ごとの実測源（span ctx・receipt・event position・probe stamp）で更新。dedup `(revision, DispatchStage)` 一致の observation は slot/key/history を更新しない。

## latency / metrics

- `Dispatched` の latency: `delivered = call_start + frame_offset`（engine frame）、`scheduled = obs.position.convert(frame_base, Rounding::NearestEven)`（不成立なら未記録）。`latency_frames = delivered - scheduled` を `MediaDuration::new(frames, frame_base).to_seconds_f64()` で f64 秒化し `last`/`max`/`samples` に記録。
- `TraceMetrics`: `events_recorded`、`history_lost`（ring 退避）、`targets_lost`（target 退避）、`expired_correlations`（gen window 外れ）、`dropped_observations`（`DispatchStage::Dropped` 出現数）、`latency_samples`/`last_latency_seconds`/`max_latency_seconds`、`last_clock: Option<MediaTimestamp>`、`last_call_start: Option<EngineFrame>`、`last_plan_generation`/`last_transport_generation`/`last_clock_epoch`。

## RT / 非 RT 境界

- RT-safe（alloc/lock/block なし、warm-up 後）: `record_configured`・`record_probe`・`trace()`（`Option<&ControlTrace>`）・`metrics()`・`history_len()`/`is_truncated()`。BTreeMap 既存 key 更新・VecDeque capacity 内 push/pop のみ。
- 非 RT: `observe_cycle`（drain Vec 確保・ledger 投影）、`dispatch`/`bind` 系、`traces()`/`history()`（Vec 返却）。
- `unsafe` なし。`#[non_exhaustive]` は公開 enum（`TraceEventKind` 等）に付与。

## Risks and Trade-offs

- `revision == dispatch ordinal` は upstream `record_plan` の現行規約への依存 — doc 明記・将来の upstream 変更で integration slice が再検証。
- receipt は node 単位で target 帰属は plan_generation 一致のみ — 別 plan の receipt は slot に入らず Missing/据置（誤帰属しない）。
- `plugin_reported` stamp は `Estimated` — 実 report channel 実装時に真の stamp 供給へ置き換える余地を残す（adapter API は `PluginReportedEvidence` をそのまま保持）。
- key position 凍結は保守的 — live 追従が必要な consumer は observe 周期ごとに query する責務。

## Verification Notes

- `tests/trace_c3_05.rs`: 実 `PluginNode`+`GraphRuntime` 駆動の fake in-process dispatch で stage 記入・receipt 相関・bounded history/lost・Expired・PlanReplaced/Dropped を検証。
- `tests/trace_c3_05_rt.rs`: 単一 `rt_` test + `stats_alloc` Region で hot path 0 alloc/0 dealloc を実測。
- canonical: `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`。
