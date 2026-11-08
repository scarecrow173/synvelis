---
id: "DESIGN-106SF03Dz2aBc3dEf4gHi5jKl6"
type: "design"
status: "approved"
title: "SF-03 slice2 設計 — RoutingFault 18分類・Last-good/Retry・reconciliation"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
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
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
  derived-by:
    - "docs/plans/sf-03-slice2-fault-reconcile.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice2-fault-reconcile.md"
  references: []
---

# SF-03 slice2 設計

## 1. `RoutingFault` 最終形（18 variant・Issue #106 §5 と 1:1）

`synvelis_project::routing_build::RoutingFault` を以下の 18 variant
に再構成する。`Copy`・fieldless を維持。slice 1 variant は右列の
対応で統合・改名される。

| variant | slice 1 からの対応 | mint 経路（実装位置） |
| --- | --- | --- |
| `StaleProjectSnapshot` | `StaleProjectRevision` 改名 | `checked_key` expected≠revision、`publish` revision/project_id 不一致 |
| `SpeculativeMismatch` | 新規 | `publish` で `routing_fingerprint(project) != key.routing_fingerprint`（revision 一致下の内容差異） |
| `PluginCapabilityChanged` | `StaleCapability` の capability 側に縮小 | `publish` で `port_capability_generation` 不一致 |
| `MissingSource` | 既存 | `SemanticLowerError` ではなく `prepare` の skipped 昇格（`SkippedPlayback::MissingSource`、comp `CompSkipReason::MissingSource`） |
| `PluginUnavailable` | 既存（`MissingProcessorInstance` 吸収） | compile `MissingProcessor`、chain instance 未供給 |
| `PortUnresolved` | 新規 | `SemanticLowerError::PortUnresolved` — `emit_processor_node`/`connection_ends` で `SemanticPortId` が port entity に解決不能 |
| `UnsupportedFormatOrTimeMap` | `UnsupportedMapping`+`ChannelMismatch` 統合 | `SemanticLowerError::{ChannelMismatch,UnsupportedDomain}`、skipped `RateMismatch`/`UnresolvableAnchor`/`UnsupportedMapping`/comp 同系、reconcile の engine/source-format generation drift |
| `InvalidRouting` | `ValidationFailed`+`CompileFailed` 統合 | semantic 構造不正（`MissingStrip`/`MasterMissing`/`DanglingGainTarget`）、非 cycle validation、clip 数不整合、非 resource/plugin compile 拒否 |
| `Cycle` | 新規 | `project_rt.prepare` の `ValidationError::{ZeroDelayCycle,DelayedCycleUnsupported}` |
| `CompileResourceExceeded` | 新規 | `SemanticLowerError::BudgetExceeded`、compile `{Slot,DelayLine,Delay,Processor}Overflow`、`checked_key` の `block_frames > max_frames` |
| `PublishQueueFull` | `PublishChannelFull` 改名 | `GraphRuntime::publish` Err（command/event channel 満杯） |
| `ActivationNotObserved` | 新規 | `reconcile` で pending が frames>0 処理済みかつ `active_generation != pending.generation` |
| `RetireOverflow` | 新規 | `reconcile` で `retire_overflows`/`plan_retire_overflows` counter 前進 |
| `DeviceEpochChanged` | 新規（旧 `ValidationFailed` の instance/epoch 経路） | `checked_key` で `runtime_instance`/`device_clock_epoch` 不一致、reconcile の `DeviceEpoch` |
| `Superseded` | 新規 | `publish` で `compiled.plan.generation <= published_plan().generation`（順序違反） |
| `BadSourceMap` | `TemporaryIdMismatch`+`GenerationMismatch` 吸収 | `publish` の source map/key/generation 内部不整合、reconcile の `SourceMapIdentity` |
| `EventStale` | `StaleEvent` 改名 | `reconcile` で `stale_events`/`stale_commands` counter 前進 |
| `OfflineCapabilityMismatch` | 新規 | `BuildMode::Offline` prepare で使用 chain instance の `offline_support() != Verified` |

除去 variant: `StaleProjectRevision`/`StaleCapability`/`ChannelMismatch`/
`UnsupportedMapping`/`ValidationFailed`/`CompileFailed`/
`MissingProcessorInstance`/`EngineStopped`/`TemporaryIdMismatch`/
`GenerationMismatch`/`PublishChannelFull`/`StaleEvent`。

## 2. `SemanticLowerError` 改修

- `PortUnresolved { port: SemanticPortId }` を追加。
  - `emit_processor_node`: `processor.port_ids` 要素が
    `routing.ports` に存在しない → `PortUnresolved`。
  - `connection_ends`: `conn.source`/`conn.destination` が
    `routing.ports` に存在しない → `PortUnresolved`。
- `fault()` 写像: `ChannelMismatch`/`UnsupportedDomain` →
  `UnsupportedFormatOrTimeMap`、`PortUnresolved` → `PortUnresolved`、
  `MissingStrip`/`MasterMissing`/`DanglingGainTarget` → `InvalidRouting`、
  `BudgetExceeded` → `CompileResourceExceeded`。

## 3. `RoutingEngine::prepare` 改修

順序（決定的）:

1. `checked_key` — `StaleProjectSnapshot`（expected≠revision）→
   `DeviceEpochChanged`（instance/epoch）→ `CompileResourceExceeded`
   （`block_frames > max_frames`）。
2. generation 予約（失敗時は gap のみ、重複なし）。
3. `BuildMode::Offline` のとき `lowered.node_roles` の
   `ChainProcessor` 各 instance を `deps.processors` から引き
   `offline_support() == OfflineRenderSupport::Verified` を要求。
   未供給は compile の `PluginUnavailable` に委ねる。
4. `lower_semantic` → `e.fault()`。
5. `deps.clips.len() != lowered.playbacks.len()` → `InvalidRouting`。
6. **skipped 昇格**: `lowered.skipped` を走査し、spec R4 の写像で
   最初の fault を `prepare` 失敗とする（Realtime/Offline 共通）。
   意図的無音 3 variant は許容。
7. `stage_build` → clip 尽き → `InvalidRouting`。
8. `project_rt.prepare` → `ValidationError` に cycle 系があれば
   `Cycle`、他は `InvalidRouting`（`PrepareError` は non_exhaustive、
   wildcard → `InvalidRouting`）。
9. `published()` None → `InvalidRouting`。
10. `compiler.compile` → `MissingProcessor`→`PluginUnavailable`、
    `*Overflow`→`CompileResourceExceeded`、その他→`InvalidRouting`。

## 4. `RoutingEngine::publish` 改修

順序:

1. `key.project_revision != revision || project.project_id != key.project_id`
   → `Blocked{StaleProjectSnapshot}`。
2. `routing_fingerprint(project) != key.routing_fingerprint`
   → `Blocked{SpeculativeMismatch}`。
3. `port_capability_generation(project) != key.capability_generation`
   → `Blocked{PluginCapabilityChanged}`。
4. **BadSourceMap 内部整合**: `prepared.generation`・
   `prepared.source_map.generation` が `compiled.plan.generation` と
   一致し、`source_map.build_key == key`・`source_map.project_id`・
   `source_map.project_revision` が key と一致すること。不一致 →
   `Blocked{BadSourceMap}`（`PreparedRouting` は pub field 構成のため
   呼出側が不整合 bundle を作り得る境界検査）。
5. **Superseded**: `compiled.plan.generation <=
   self.graph.published_plan().generation` → `Blocked{Superseded}`。
   旧世代 plan が新世代を displacement することを防ぐ。
6. `graph.publish` → Err → `Failed{PublishQueueFull}`。
7. 成功 → staged state commit・`published_request_id` 記録・
   pending 差替・`Published` receipt。

## 5. `RoutingEngine::reconcile`（新規 public API）

```rust
pub fn reconcile(
    &mut self,
    project: &Project,
    revision: ProjectRevision,
    engine_config_generation: u64,
    source_format_generation: u64,
) -> RoutingReconcileReport
```

`RoutingReconcileReport`（`routing_build.rs` 新規）:

```rust
pub struct RoutingReconcileReport {
    pub plan_key: Option<RoutingBuildKey>,
    pub mismatches: Vec<RoutingReconcileMismatch>,
    pub fault: Option<RoutingFault>,
}
```

`RoutingReconcileMismatch`（Copy enum）:
`SourceMapIdentity`/`ProjectRevision`/`RoutingFingerprint`/
`CapabilityGeneration`/`EngineConfigGeneration`/
`SourceFormatGeneration`/`DeviceEpoch`/`RuntimeCounters`/
`PendingActivation`。

手順:

1. `self.source_map` が None → 空 report（plan 未 publish、照合対象なし）。
2. `plan_key = source_map.build_key`。identity 検査:
   `project.project_id == key.project_id`、
   `source_map.project_id/project_revision == key`、
   `source_map.generation == ledger.published_generation`。
   いずれか不一致 → `SourceMapIdentity`。
3. CAS/semantic: `key.project_revision != revision` →
   `ProjectRevision`。`routing_fingerprint(project) !=
   key.routing_fingerprint` → `RoutingFingerprint`。
   `port_capability_generation(project) != key.capability_generation`
   → `CapabilityGeneration`。引数の engine/source-format generation
   不一致 → 各 mismatch。
4. `self.epoch != key.device_clock_epoch || self.instance !=
   key.runtime_instance` → `DeviceEpoch`。
5. RT counter: `rt_status()` snapshot の `retire_overflows`/
   `plan_retire_overflows`/`stale_events`/`stale_commands` を Ledger
   baseline と比較。retire 系前進 → `RuntimeCounters`（fault は
   `RetireOverflow`）、stale 系前進 → `RuntimeCounters`（fault は
   `EventStale`）。baseline は照合の有無に関わらず毎回更新する。
6. pending: `ledger.pending` があり `p.frames > 0` かつ
   `snapshot.active_generation != p.generation` →
   `PendingActivation`（fault `ActivationNotObserved`）。
   `frames == 0` は Playing/Stopped いずずれでも過渡的で fault なし。
7. fault 優先順位（先勝ち、spec R5 と同一）で 1 つ選び、
   `Blocked{fault, last_good_generation}` を receipt に記録
   （`request_id` は `published_request_id`）。mismatch ゼロなら
   receipt 不変で report のみ返す。

## 6. Ledger 拡張

```rust
struct Ledger {
    receipt: Option<RoutingActivationReceipt>,
    pending: Option<PendingActivation>,
    last_good_generation: Option<u64>,
    planned_generation: Option<u64>,
    published_generation: Option<u64>,
    published_request_id: Option<RoutingBuildRequestId>, // 新規
    observed_frames: u64,                                // 新規: Active 時の frames
    last_stale_events: u64,                              // 新規 baseline
    last_stale_commands: u64,
    last_retire_overflows: u64,
    last_plan_retire_overflows: u64,
}
```

- `observe_activation` で pending クリア時に
  `observed_frames = pending.frames` を保存。
- `record_state` の `frames_observed` は
  `pending.frames` があればそれ、なければ `observed_frames`。
- `publish` 成功時に `published_request_id = Some(request_id)`。

## 7. `RoutingDeps`/caller 影響

- `RoutingDeps`・`prepare`/`publish`/`run` の署名は不変。
- `reconcile` は新規追加のみ。既存 caller（tests, 将来の app 層）
  への破壊的変更なし。
- `RoutingFault` variant 改名は `Copy` enum の網羅 match を持つ
  caller に影響するが、workspace 内の使用は routing_engine/
  routing_lower/tests のみ（grep 確認済み）。

## 8. 変更ファイル

- `crates/synvelis-project/src/routing_build.rs` — `RoutingFault`
  再構成、`RoutingReconcileMismatch`/`RoutingReconcileReport` 追加。
- `crates/synvelis-player/src/routing_lower.rs` —
  `SemanticLowerError::PortUnresolved` 追加・`fault()` 写像更新・
  3 箇所の error 置換。
- `crates/synvelis-player/src/routing_engine.rs` — prepare/publish
  改修、`reconcile` 追加、Ledger 拡張、frames_observed 修正。
- `crates/synvelis-player/tests/sf03_l2.rs`（新規）— slice 2 受入
  テスト。`sf03_l1.rs` は variant 改名に伴う assert 更新のみ。
- `docs/` — 本チェーン（spec/design/plan/task/test-spec）。

## 9. リスクと対応

- **variant 改名の波及**: workspace 内使用箇所は限定的（§7）。
  `cargo check` で残存を検出。
- **skipped 昇格による realtime 挙動変化**: slice 1 では skipped が
  捨てられ fake silence になり得た。spec R4 は Issue の
  「fake silence 禁止」に基づく意図的な fail-closed 化。意図的無音
  3 variant は維持。
- **reconcile の pending 判定**: `frames>0 && observed!=gen` は
  observe_activation が毎 block 照合するため、通常は pending が
  即クリアされ本条件に達しない。到達するのは RT が plan を
  activate しなかった異常系のみで、`ActivationNotObserved` が
  正しい分類。
- **counter baseline**: reconcile を呼ばない期間の counter 前進も
  次回 reconcile で検出される（baseline は前回値比較）。
- **現行入力では発火しない防御的分類**: `port_capability_generation`
  は `project.routing` 内の `capability_revision` の和であり、
  `routing_fingerprint` が `project.routing` 全体を被覆するため、
  capability drift は必ず fingerprint drift を伴う。publish /
  reconcile とも fingerprint を先に検査するので、現行データモデル
  では `PluginCapabilityChanged` は `SpeculativeMismatch` に
  shadow される。variant と mint 経路は Issue §5 の 18 分類を
  完備するために配線済みで、live capability generation が
  project 外の plugin host 状態を含むようになった時点（SF-ACT
  系の後続 slice）で実際に発火し得る。`reconcile` の `DeviceEpoch`
  も同様に、engine の epoch/instance が構築後不変な現行構成では
  防御的経路。`RetireOverflow` は retire channel 溢れという
  Ruviform 側の異常が前提で、headless harness からは決定的に
  再現できないため reconcile 経路の分類として配線するに留める。
