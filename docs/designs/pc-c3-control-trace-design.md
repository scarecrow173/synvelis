---
id: "DESIGN-034UqtGwJKTXdIjOxuLVS2"
type: "design"
status: "approved"
title: "PC-C3 ControlTrace 設計 — stage evidence slot・freshness 判定・plan activation 投影"
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
    - "docs/adr/pc-c3-control-trace-stage-evidence-freshness.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c3-control-trace-projection.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C3 ControlTrace 設計 — stage evidence slot・freshness 判定・plan activation 投影

## Context

`crates/synvelis-project` に read-only の ControlTrace projection を追加する（Issue #100・TRACE-C3-01..04）。依存境界:

- `synvelis-project` の依存は `ruviform-media-core` / `ruviform-plugin-core` / `ruviform-graph` 等。**`ruviform-audio-plugin-bridge` には依存しない**ため、`DispatchStage`/`DropReason`/`ParamValueObservation`/`NodeReport` は semantic mirror 型として trace.rs 内に定義する（意味論を合わせる。bridge 型への直接依存追加は Cargo.toml 変更を伴い本 slice の編集範囲外）。
- `AppliedReceipt`/`ApplyStatus`/`ScheduleFault`/`FaultKind`/`ClockEpochId`/`TransportGeneration`/`PlanGeneration`/`EngineFrame`/`MediaTimestamp`/`TimestampProvenance`/`LoopOccurrence` は `ruviform-media-core` から利用可能。
- `ParamValue`（normalized [0,1]）は `ruviform-plugin-core` から利用可能。
- `RoutingFault`/`RuntimeInstanceId`/`RuntimeClockEpochRef` は `crate::routing_build`（同 crate）から再利用。
- `crate::id::ClockEpochId`（project 永続 epoch）と `ruviform_media_core::ClockEpochId`（runtime epoch）は別物 — trace は runtime 観測証拠なので media-core 側を使い、import は alias で衝突回避。

## Scope

### In Scope

- `crates/synvelis-project/src/trace.rs`（新規）: 全 projection 型 + freshness 評価関数。
- `lib.rs` additive export、`tests/trace_c3.rs` fixture test、docs チェーン。

### Out of Scope

- TRACE-C3-05・workspace/player 統合・#101/#103/#114・実 plugin/OS/RT 観測（spec 参照）。

## Components and Boundaries

```
trace.rs
├── ControlTraceKey            — 証拠帰属の相関キー（8 field、下記）
├── TraceSlot<T>               — Present / Missing / Unsupported / Dropped / Expired
├── ControlTrace               — key + 7 slot
│     configured:   TraceSlot<ConfiguredEvidence>
│     prepared:     TraceSlot<PreparedEvidence>
│     scheduled:    TraceSlot<ScheduledReceipt>
│     applied:      TraceSlot<AppliedStageReceipt>
│     dispatched:   TraceSlot<DispatchEvidence>
│     plugin_reported: TraceSlot<PluginReportedEvidence>
│     observed:     TraceSlot<ProbeObservation>
├── stage evidence 型群         — ConfiguredEvidence / PreparedEvidence /
│                                 ScheduledReceipt / AppliedStageReceipt /
│                                 DispatchEvidence(+DispatchDropReason) /
│                                 PluginReportedEvidence / ProbeObservation(+MeasuredValue)
├── FreshnessContext           — 現在世代・epoch・position・now・max_age
├── ObservationFreshness       — Current/Estimated/Stale{reason}/Unavailable{reason}/DataLoss{count}
├── StaleReason / UnavailableReason
└── PlanActivationState        — active + last_attempt 分離（failed plan 非 active）
```

## Key and Slot Model

`ControlTraceKey`（issue 指定 + loop disambiguation）:

| field | 型 | 意味 |
| --- | --- | --- |
| `project_id` | `ProjectId` | 所有 project |
| `parameter_target_id` | `ParameterTargetId` | semantic 対象（raw node/param id ではない） |
| `project_revision` | `ProjectRevision` | committed revision |
| `control_plan_generation` | `PlanGeneration` | dispatch plan 世代（0 = 未 stage） |
| `graph_generation` | `u64` | SF-03 `RoutingSourceMap.generation` 相当の graph build 世代 |
| `transport_generation` | `TransportGeneration` | Play/Pause/Stop/Seek で bump（Loop wrap は bump しない） |
| `clock_epoch` | `ruviform_media_core::ClockEpochId` | device clock epoch（DeviceRestart で bump） |
| `loop_occurrence` | `LoopOccurrence` | loop wrap ごとに increment — 同一 position の別 pass を区別 |
| `evaluation_position` | `MediaTime` | 評価位置（cross-base Ord で比較） |

`TraceSlot<T>`: `Present(T)` / `Missing`（最初から無い）/ `Unsupported`（この target では生成不能）/ `Dropped`（相関前に喪失）/ `Expired`（相関前に失効）。欠落 stage を合成しない。

Receipt slot の型安全性: `ScheduledReceipt::new(ReceiptEvidence)` は `status ∈ {Requested, Pending}` のみ `Some`、`AppliedStageReceipt::new` は `status == Applied` のみ `Some` — 誤 status の証拠を slot に入れられない（fail-closed）。`ReceiptEvidence` は RT-minted `AppliedReceipt` から projection される証拠 payload（`CommandId` 相関 token は scheduler 内部専用のため保持しない）。`AppliedReceipt` は `CommandId` を外部から構築できないため fixture でも直接生成不能 — projection 型として `ReceiptEvidence` を採用した。

`DispatchEvidence` は bridge `DispatchStage` の semantic mirror: `Scheduled`/`Dispatched{call_start: EngineFrame, frame_offset}`/`Dropped{reason: DispatchDropReason}`/`Reported`/`Unavailable`。`Dispatched` は計画/スパンへの投影であり plugin ACK ではない（bridge module docs の honesty contract を継承）。`DispatchDropReason` は `DropReason` の mirror（`EventScratchOverflow`/`EndpointError{kind}`/`UndeliverableSpan`/`ScheduleFault`/`PlanReplaced`/`VaultExceeded`）。

## Freshness Evaluation

`FreshnessContext`:

```rust
pub struct FreshnessContext {
    pub project_revision: ProjectRevision,
    pub control_plan_generation: PlanGeneration,
    pub graph_generation: u64,
    pub transport_generation: TransportGeneration,
    pub clock_epoch: ClockEpochId,        // media-core runtime epoch
    pub evaluation_position: MediaTime,
    pub loop_occurrence: LoopOccurrence,
    pub now: MediaTime,                   // 現在 playhead（age 判定基準）
    pub max_age: MediaDuration,           // age budget
}
```

`ControlTrace::freshness(ctx)`:

1. **key↔ctx 世代整合**（先に評価 — 不一致は trace 全体の stale）:
   `project_revision` → `Stale{ProjectRevision}`、`control_plan_generation` → `Stale{ControlPlan}`、`graph_generation` → `Stale{Graph}`、`transport_generation` → `Stale{Transport}`（Seek/Pause/Play/Stop。Loop wrap は `loop_occurrence` 差で `Stale{Loop}`）、`clock_epoch` → `Stale{ClockEpoch}`（DeviceRestart）、`evaluation_position`/`loop_occurrence` → `Stale{Position}`。最初に見つかった不一致を返す（決定論的順序）。
2. **channel freshness**: `plugin_reported` と `observed` を独立評価し、`freshness()` は良い方（rank: Current < Estimated < Stale < DataLoss < Unavailable）を返す。同 rank の tie は「slot が Present（証拠あり・untrusted 含む）を優先、さらに probe channel 優先」。個別には `plugin_reported_freshness()` / `observed_freshness()`。
3. **stamp 評価**（`MediaTimestamp` 保持 channel のみ）:
   - `stamp.time.checked_add(max_age)` で deadline 算出（overflow → `Unavailable{ClockUntrusted}`）
   - `now > deadline` → `Stale{Age}`
   - `now < stamp.time`（未来 stamp）→ `Unavailable{ClockUntrusted}`
   - `provenance == Estimated` → `Estimated`（Current にしない）
   - それ以外 → `Current`
4. **observed channel**: `ProbeObservation::Lost{count}` → `DataLoss{count}`、`Missing` → `Unavailable{ProbeMissing}`、`Disabled` → `Unavailable{ProbeDisabled}`、`Measuring{stamp, value}` → stamp 評価。`MeasuredValue::Silence` は `Measuring` と同じ経路で評価される（実測無音 ≠ probe 欠落）。
5. **slot 非 Present** → `Unavailable{NoEvidence|Unsupported|Dropped|Expired}`（slot 状態を reason に写像）。

`StaleReason`: `ProjectRevision`, `ControlPlan`, `Graph`, `Transport`, `ClockEpoch`, `Loop`, `Position`, `Age`。
`UnavailableReason`: `NoEvidence`, `Unsupported`, `Dropped`, `Expired`, `ClockUntrusted`, `ProbeMissing`, `ProbeDisabled`, `OutOfRange`。

## Plan Activation Projection

```rust
pub struct PlanActivationState {
    /// 現在 active と投影される plan（last-good — 失敗 publish を跨いで継続）。
    pub active: Option<ActivePlan>,
    /// 直近の失敗試行 — active には決してならない。
    pub last_attempt: Option<FailedPlanAttempt>,
}
pub struct ActivePlan {
    pub plan_generation: PlanGeneration,
    pub transport_generation: TransportGeneration,
    pub clock_epoch: ClockEpochId,
    pub runtime_instance: RuntimeInstanceId,
}
pub struct FailedPlanAttempt {
    pub plan_generation: PlanGeneration,
    pub fault: RoutingFault,   // 既存 18-variant taxonomy を再利用
    pub transport_generation: TransportGeneration,
    pub clock_epoch: ClockEpochId,
}
```

`active` と `last_attempt` を別 field にすることで「失敗 plan が旧 active を置き換えない」が型レベルで保証される。`RoutingActivationState::Failed{last_good_generation}`/`RoutingFault`/`ExecutionEquivalence` の意味論と整合。`routing_build.rs` は変更しない — 本型は param-dispatch 経路の plan activation を投影するもので、routing publish とは別経路。

## Receipt Coherence

`ControlTrace::is_coherent() -> bool`: slot 内 evidence が埋め込む generation（scheduled/applied receipt の `plan_generation`/`transport_generation`/`clock_epoch`、dispatched evidence の `plan_generation`）が key と一致するかを検査。`false` の trace は「証拠が別 context のもの」— consumer は freshness/stage 証拠を信用してはならない。freshness() とは独立（観測 channel は generation を持たないため）。

## Data and Control Flow

- **生成**: 後続 integration slice が project state・dispatch ledger・probe receipt を読み、本型を構築する。本 slice は構築 helper を持たない（型 + freshness 関数のみ）。
- **消費**: workspace `ControlSession`/UI/Oracle が `freshness()`・`is_coherent()`・slot を読む。read-only・決定論・alloc は freshness 評価で行わない（比較のみ）。

## Non-Functional Constraints

- RT 安全性: 本 module は非 RT projection 層。alloc は型構築時のみ（`Vec` 不使用 — `is_coherent` は bool）。freshness は `Copy` 比較のみ。
- 決定論: 全比較は `Ord`/`PartialEq` の exact 比較。`MediaTime`/`MediaDuration` の算術は checked のみ（`checked_add`）。
- serde: relation.rs 先例に倣い derive しない（runtime evidence projection であり永続化対象外）。
- `#[non_exhaustive]`: 公開 enum は crate policy に従い付与（`TraceSlot`/`DispatchEvidence`/`DispatchDropReason`/`ProbeObservation`/`MeasuredValue`/`ObservationFreshness`/`StaleReason`/`UnavailableReason`）。

## Risks and Trade-offs

- **bridge 非依存の mirror 型**: `DispatchStage` 意味論の drift リスク → doc で mirror 関係を明記し、integration slice で変換時に検証。
- **freshness() が best-channel を返す**: probe Lost + plugin Current → `Current`。`observed_freshness()` で DataLoss を個別取得可能 — doc に明記。
- **position 厳密一致**: `evaluation_position`/`loop_occurrence` の不一致を `Stale{Position}`/`Stale{Loop}` とするのは保守的（誤 Live 防止優先）。automation が position 連続的に変わる target では consumer が適切な key で query する責務。

## Deferred Design Concerns

- TRACE-C3-05（bounded history・実計測 RT adapter）→ synvelis-player slice。
- workspace 接続・single drain owner・2 Inspector 二重起動防止 → integration slice（`ProbeObservation::Missing`/`Disabled` が判断材料）。
- #101 Binding/FrameBase・#103 Oracle・#114 GPUI → 各 Issue gate。

## Verification Notes

- `tests/trace_c3.rs` で TRACE-C3-01..04 を fixture 検証: slot 独立性・Missing 維持・generation/epoch/position/age 分類・probe 状態区別・failed plan 投影・`is_coherent`。
- `mise run fmt-check` / `check` / `lint` / `test` を canonical 検証とする。
