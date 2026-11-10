---
id: "DESIGN-034Ux9PPNPAMU9m3oovJh8"
type: "design"
status: "approved"
title: "SF-03 semantic→Ruviform runtime 写像 — 責務境界・世代証拠・失敗不変条件・検証計画"
created: "2026-11-09"
updated: "2026-11-10"
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
  implements:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-02-routing-transaction-design.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
    - "docs/discovery/sf-03-map-current-state-inventory.md"
  derived-by:
    - "docs/plans/sf-03-map-semantic-runtime-mapping-docs.md"
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
    - "docs/designs/sf-03-slice6-act06-control-surface-design.md"
    - "docs/specs/sf-03-slice7-render-consumption.md"
    - "docs/designs/sf-03-slice7-render-consumption-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-map-failure-invariant-verification.md"
  references: []
---

# SF-03 semantic→Ruviform runtime 写像 — 責務境界・世代証拠・失敗不変条件・検証計画

本書は Issue #106 の semantic→Ruviform runtime 写像の**統合設計権威**。
slice1〜5 の実装済み契約を再掲せず、その上に consumer 横断の
世代証拠契約・失敗不変条件・検証計画を固定する。実装は対象外。

## 1. 責務境界（crate/型 owner 固定）

| 層 | Owner（path） | 責務 | やらないこと |
| --- | --- | --- | --- |
| Semantic 正本 | `synvelis-project`（`routing.rs`/`routing_tx.rs`/`controller.rs`） | `RoutingModel`・`ProjectRevision`・CAS・semantic validation（SF-01/02） | runtime ID の生成・永続化 |
| 契約型 | `synvelis-project`（`routing_build.rs`/`trace.rs`/`probe.rs`/`probe_trace.rs`） | `RoutingBuildKey`/`RoutingSourceMap`/`RoutingFault`(18)/`RoutingActivationState`/`Receipt`/`ReconcileReport`/`ExecutionEquivalence`、`ControlTrace`、probe semantic contract | audio runtime 依存・RT 処理 |
| Lowering | `synvelis-player`（`routing_lower.rs`/`lowering.rs`） | `RoutingModel`→`GraphSpec`+`RoutingSourceMap`+node role、`plan_playbacks`、`StripProcessor` | publish/RT 駆動・plugin lifecycle |
| Activation/Plan | `synvelis-player`（`routing_engine.rs`/`host.rs`/`processor.rs`） | `prepare→publish→process→observe_activation→reconcile`、last-good ledger、`ProcessorHost` boundary、probe passthrough | semantic 正本の変更・Project CAS |
| Session/Workspace | `synvelis-workspace`（`EnginePort`/`ControlSession`/`signal_flow_ui`/`relation_ui`） | revision 通知・probe drain 単一 owner・read model 投影 | engine 内部状態の複製 |
| App composition | `synvelis-app`（`SessionRoutingEngine`） | deps 組立・ops・UI 向け evidence snapshot | lowering/activation ロジックの再実装 |
| Offline Render | `synvelis-render` | offline render job・PDC pass・codec/format | semantic lowering の別実装 |
| Ruviform（generic） | `ruviform-graph`/`ruviform-audio::typed`/`ruviform-plugin-host`/probe broker | Typed Graph validator・Compiler・`GraphRuntime::publish`・`RtRuntime::process`/`active_generation`・Slot/PDC・plugin host lifecycle・probe rebind/clock | —（Synvelis は複製しない） |

**境界不変条件**: runtime `NodeId`/`PortId`/`ConnectionId`/`ProbeId`/
`PluginHandle`/plan `generation` は graph-local・runtime-scoped であり、
Project に永続化しない。Project が保持するのは安定 semantic ID
（`StripId`/`BusId`/`ProcessorId`/`SemanticPortId`/`RoutingConnectionId`/
`SendId`/`SemanticProbePoint`）のみ。

## 2. 世代モデルと証拠分離

### 2.1 世代名前空間

| 名前空間 | Owner | 意味 | 永続化 |
| --- | --- | --- | --- |
| `ProjectRevision` | `synvelis-project` | semantic configured 状態（CAS 権威） | 永続化 |
| `RoutingFingerprint` + capability/engine/source-format generation | `synvelis-project` | semantic 等価・stale 判定の構成要素 | Project 由来（導出値） |
| plan `generation`（`CompiledPlan.generation`） | `synvelis-player`/Ruviform runtime | Prepared/Published の証拠。`RuntimeInstanceId` 内で単調増加 | 非永続（runtime-scoped） |
| `active_generation`（`SharedStatus`） | Ruviform `RtRuntime` | RT が実際に駆動している世代 | 非永続 |
| `RuntimeInstanceId`/`RuntimeClockEpochRef` | engine/session | runtime 実行文脈。device restart で更新 | 非永続（`ClockEpochId` と無条件同一視しない） |

`RoutingBuildKey` は上記を束ね、`semantic_eq` は `project_revision`
のみを除く同値比較を提供する（NoRebuild 判定用）。

### 2.2 Activation 状態機械（`RoutingActivationState`）

```
Configured{rev} → Validated{rev} → Prepared{rev,planned_generation}
    → Published{rev,gen} → Active{rev,gen,epoch}
         ↘ Blocked/Failed{fault,last_good_generation}
         ↘ Retired（superseded/retired）
```

- `Published` は非 RT 証拠（`GraphRuntime::publish` 成功点）。
- `Active` は RT `process(frames>0)` が `active_generation ==
  published gen` を観測した時点でのみ mint。
- `last_good_generation` = 最後に RT 観測された Active 世代。
  失敗経路・reconcile drift・superseded で後退しない。
- Pending 明示: `Published` のまま RT 未観測の期間は
  `Configured R_new / Published|Active R_old(gen G_old)` を
  receipt + generation で区別可能。engine は `PendingActivation`
  ledger で frames を累積し、overdue は reconcile の
  `PendingActivation`/`ActivationNotObserved` mismatch で検出する。
  経過時間だけで成功/失敗を断定しない。
- `EngineStub`（workspace 既定 `EnginePort`）は graph build 回数を
  数えるだけで activation receipt を生成しない — consumer は
  `graph_builds` 等の build 計数を Active 証拠として扱わない。

### 2.3 TRACE-C3 との整合

`ControlTraceKey`（`project_revision`/`control_plan_generation`/
`graph_generation`/`transport_generation`/`clock_epoch`/
`evaluation_position`）の `graph_generation` は本書の plan
`generation` 名前空間を指す。`ObservationFreshness` は
graph/plan/epoch/transport/position の不一致を `Stale`/
`Unavailable` に分類し、誤 Live を出さない。SF-03 の receipt/
SourceMap と ControlTrace は**同一世代名前空間**を共有し、
consumer は独自の active 判定を持たない（ADR 決定）。

## 3. SourceMap 契約

- `RoutingSourceMap` は `lower_semantic` と同時に生成され
  `PreparedRouting` に同梱。内容: `semantic_nodes`/`ports`/
  `connections`/`playback_bindings`/`send_taps` + `build_key` +
  `generation` + `project_id`/`project_revision`。
- publish 時に `generation`/`build_key`/`project_id`/
  `project_revision` の内部整合を検査（`BadSourceMap`）。
- 失効条件: plan swap・plugin reload/reattach・port reorder・
  generation 交代。失効後の旧 map 経由の解決は行わない。
- consumer は**publish 済み plan の SourceMap** を読む:
  - probe: `resolve_probe_point(project, map, spec, point)` が
    `RoutingSourceMap`+`GraphSpec` 経由で fail-closed 解決
    （`Unavailable`/`Unsupported`/`Unresolved` を正直に返す）。
  - control: `take_processor_control(generation, processor_id)`
    が公開済み世代の control のみ one-shot 払い出し。
  - PCM render: `playback_bindings`/`RoutingPlaybackRef` が
    semantic content→runtime node/out port を対応付け。
  - UI: SF-05 `SignalFlowRuntimeEvidence` が receipt/source_map/
    probe bindings の snapshot を read-only 消費。

## 4. Lowering・PDC 意味論

### 4.1 決定的 lowering 順序（`lower_semantic`）

1. `routing.strips` を BTreeMap 順に走査。各 strip に fader node
   （main-in/pre-out/post-out）+ chain processor node（chain 順）。
2. `routing.connections` を BTreeMap 順に走査し enabled のみ edge
   化: `Main`（strip post-out→dst main-in）、`Send`（pre/post tap
   →send gain node→dst main-in）、`Sidechain`（strip post-out→
   processor sidechain in）。
3. `plan_playbacks` の各 playback を content→lane→track→strip で
   所有 strip の main-in へ接続。
4. master strip post-out→device sink（`device_in`）。

信号順（strip 内）: main-in → insert chain（宣言順）→ fader/pan →
post-out。`pre-out` は入力 dry copy（PreFader send tap 用）。

### 4.2 Mute/Solo・tap・channel map

- mute/solo は `audible_strips` が唯一の authority — edge prune +
  `NotLowered(MutedStrip)` 記録。processor 内に mute 状態を持た
  ない。
- `SendTap::PreFader`/`PostFader` は lowering 時に別 PortRef へ
  解決。`SendConfig.gain_target` 不在は gain node を emit しない。
- `ExplicitChannelMap` は推論 upmix/downmix せず明示 map。
- pan は constant-power 則、fader/send gain は target `base` の
  線形 gain（target 不在は unity/direct）。

### 4.3 PDC の境界

- semantic lowering は PDC（plugin delay compensation）を直接
  符号化しない。PDC の機構は Ruviform typed runtime の Slot/PDC
  と `synvelis-render` の offline render job が所有する。
- SF-ACT-02 の「PDC」要件の設計上の担保点は **lowered graph の
  topology が Realtime/Offline で同一**であること:
  `BuildMode::Realtime`/`Offline` は同じ `lower_semantic` を通り、
  offline は `OfflineCapabilityMismatch` gate（chain processor が
  `OfflineRenderSupport::Verified` を報告すること）を追加する。
  node/port/connection の順序・tap 位置・sidechain 分離が同一
  なら、consumer 側の PDC realignment が同じ意味で適用される。
- PDC 実効値の検証（latency 報告・realignment 精度）は
  Ruviform RT contract（`mise run test-rt`）と synvelis-render
  （L2 export）の責務 — 本書は lowering 同一性のみを規定する。

### 4.4 失敗規則（lowering）

- clip/comp source 未解決・rate 不一致・mapping 非対応は
  `SkippedPlayback` と同一規則で記録。slice2 R4 の `skipped_fault`
  昇格により、意図的無音（`Muted`/`SupersededByComp`/
  `CompSilence`）以外の skip は prepare を fault で失敗させる
  （fake silence 禁止・両 mode fail-closed）。
- `SemanticPort.channels` ≠ engine channels・enabled connection
  endpoint 未解決は lowering error（deterministic 失敗）。
- `ProcessorBinding::Unresolved`/instance 未供給は emit するが
  connect/compile factory が失敗 → `PluginUnavailable` で
  fail-closed（bypass も fake silence もしない）。

## 5. Gesture CAS・speculative plan・publish

### 5.1 gesture→commit→build の流れ

1. **Speculative 検証（commit 前）**: gesture の候補 snapshot で
   semantic validation + lowering 予備検査。候補の可否を早期に
   知るためのもので、**speculative compiled plan を publish しない**。
2. **CAS commit**: `ProjectController.apply` が expected revision
   で CAS。stale なら commit 前に reject。
3. **Committed snapshot から再照合/再 compile**: commit 成功後、
   live project から `RoutingBuildKey` を再計算し、prepared plan
   との一致を publish 直前に検証。
4. **publish→RT observe→receipt**: `GraphRuntime::publish` →
   `Published` → RT `active_generation` 観測 → `Active`。

### 5.2 publish 時の再検証（`RoutingEngine::publish`）

live project から key を再計算し、以下の不一致は publish 0 で
`Blocked`/`Failed` receipt（last-good 維持）:

| 検査 | fault |
| --- | --- |
| project_id/revision 不一致 | `StaleProjectSnapshot` |
| routing fingerprint 不一致（同一 revision での内容差異） | `SpeculativeMismatch` |
| `port_capability_generation` drift | `PluginCapabilityChanged` |
| source map/build key/generation 内部不整合 | `BadSourceMap` |
| generation ≤ last published（順序違反） | `Superseded` |
| publish channel/event 満杯 | `PublishQueueFull` |

### 5.3 reconcile（publish 前・retry 前の drift gate）

`RoutingEngine::reconcile` は最後に publish された plan の
`RoutingBuildKey`/`RoutingSourceMap` を live committed snapshot・
RT counter と再照合し、全不一致側面を
`RoutingReconcileMismatch` として記録、優先順位で 1 つの
`RoutingFault` を receipt（`Blocked`）に記録する。旧 plan は維持。

優先順位（先勝ち）: `BadSourceMap` → `StaleProjectSnapshot` →
`SpeculativeMismatch` → `PluginCapabilityChanged` →
`UnsupportedFormatOrTimeMap` → `DeviceEpochChanged` →
`RetireOverflow` → `EventStale` → `ActivationNotObserved`。

### 5.4 連続編集・retry・NoRebuild

- rapid R1/R2/R3: 各 prepare→publish。未 Active の pending 世代は
  次 publish で置き換え（lease 即時解放）。`last_good_generation`
  は RT 観測まで後退しない。
- retry は常に fresh prepare（新 BuildKey/SourceMap/generation）。
  失敗した `PreparedRouting`・旧 generation・旧 source map を
  再利用しない。
- NoRebuild: `live_key.semantic_eq(&new_key)` が真かつ revision が
  異なる場合、compile/generation 消費なしで `ExecutionEquivalence`
  を返す。新 Revision が Active になったという偽 receipt は出さ
  ない。同一 revision の再 prepare は通常 build。

## 6. Plan publish の失敗不変条件（consumer 横断）

### 6.1 `RoutingFault` 18 分類（Issue #106 §5）

`StaleProjectSnapshot`/`SpeculativeMismatch`/`PluginCapabilityChanged`/
`MissingSource`/`PluginUnavailable`/`PortUnresolved`/
`UnsupportedFormatOrTimeMap`/`InvalidRouting`/`Cycle`/
`CompileResourceExceeded`/`PublishQueueFull`/`ActivationNotObserved`/
`RetireOverflow`/`DeviceEpochChanged`/`Superseded`/`BadSourceMap`/
`EventStale`/`OfflineCapabilityMismatch`。全 mint 経路は slice2
spec の対応表どおり決定的で、未分類 fallback を作らない。

### 6.2 ユーザー指定シナリオ → 設計上の不変条件

| シナリオ | 設計上の不変条件 | fault/状態 |
| --- | --- | --- |
| 連続 Routing edits | pending 世代は次 publish で置換、last-good は RT 観測まで維持、receipt は世代別 | `Published`→`Active` or 置換 |
| multiwindow CAS | `ProjectController` CAS expected_revision で stale commit を拒否、engine 側も publish/reconcile で再検証 | `StaleProjectSnapshot`/`SpeculativeMismatch` |
| device restart | `RuntimeInstanceId`/`RuntimeClockEpochRef` が変わり旧 generation は無効、新 prepare が必要 | `DeviceEpochChanged` |
| plugin missing/reload | `PluginUnavailable` で fail-closed、capability drift は publish 拒否、世代連動 lease teardown・probe rebind segment cut | `PluginUnavailable`/`PluginCapabilityChanged` |
| publish QueueFull | command/event channel 満杯で publish 失敗、旧 plan 維持、stale command は RT 側 discard→counter 前進→reconcile `EventStale` | `PublishQueueFull`/`EventStale` |
| retire overflow | bounded retire channel 溢れは fault/resource degradation であって clean success ではない | `RetireOverflow`（reconcile counter 検出） |
| graph generation mismatch | source map/build key/generation 不整合は publish 前に拒否、publish 済み後の drift は reconcile で検出 | `BadSourceMap`/`Superseded`/`ActivationNotObserved` |
| old event wrong-target 0 | generation-tagged dispatch・probe `StaleGeneration` reject・control 世代スコープ払い出しで旧世代への配送経路を持たない | 型レベルで保証 |
| Publish success≠RT 駆動 | `Published` と `Active` を別 receipt state で保持、consumer は `observed_active_generation` のみを active 証拠とする | `PublishedAwaitingCallback` 相当の Pending |
| RT active delayed | 停止中/frames=0/経過時間で成功/失敗を断定しない、`Published` のまま Last-good と新 plan が queue 上で待機 | `Published` 継続 |

### 6.3 consumer 同一 Revision 契約（MAP-R6）

全 consumer は `ProjectRevision` + plan `generation` を相関キーと
し、独自の active 判定を持たない:

- **Probe**: `SemanticProbePoint`→`resolve_probe_point` が
  publish 済み SourceMap で解決。binding は generation 付き、
  publish 時 `RebindReceipt` で segment cut（自動継続しない）。
  stale record は `StaleGeneration` で trace 記録拒否。
- **Controls/ParamDispatch**: `take_processor_control` が公開済み
  世代の control のみ払い出し。dispatch ledger は plan generation
  を記録、trace freshness が `graph_generation` を照合。
- **PDC/render**: `BuildMode` + 同一 SourceMap topology。
  `OfflineCapabilityMismatch` gate。
- **PCM render**: `playback_bindings` が publish 時に install され、
  旧世代の playback は retire と共に解放。
- **Undo/Save**: undo/redo は新 revision → 通常 rebuild/equivalence
  判定。Save は semantic model のみ永続化（runtime ID を含まない）。

## 7. SF-ACT-01/02/03/04 設計確定

| 受入行 | 設計上の確定内容 | 実装 slice | 検証 fixture（headless） |
| --- | --- | --- | --- |
| SF-ACT-01 Legacy direct→master vs 新 Routing Golden PCM/加算順序/Insert/Comp・SourceMap・Missing Source 明示 | `lower_semantic` は legacy `lower` と同一の clip/comp source window 解決（`plan_playbacks` 共有）・同一 fan-in sum 機構。SourceMap↔revision 一致、Missing Source は `SkippedPlayback`→`skipped_fault` 昇格で明示 | slice1（PR #191）+ slice2 fault 昇格（PR #192） | `sf03_l1.rs` golden PCM・`sf03_l2.rs` fault 昇格 |
| SF-ACT-02 Pre/Post/Sidechain/Mute/Solo/Return/Bus/Processor 順序・PDC・RT/Offline 同意味 lowering | §4.1〜4.3 の決定的順序・tap 分離・`audible_strips` prune・`BuildMode` 同一 lowering + `OfflineCapabilityMismatch` gate。PDC topology 同一性 | slice1 + slice2 R6 | `sf03_l1.rs` 順序/tap、`sf03_l2.rs` offline gate |
| SF-ACT-03 Gesture Candidate≠Committed Snapshot・CAS stale・Plugin Cap drift・temporary ID mismatch で Speculative Plan publish 0 | §5.1〜5.3 の speculative→CAS→再照合→publish、`SpeculativeMismatch`/`StaleProjectSnapshot`/`PluginCapabilityChanged`/`BadSourceMap` で publish 0、reconcile drift gate | slice1 publish 再検証 + slice2 reconcile + slice4 NoRebuild | `sf03_l1.rs`/`sf03_l2.rs`/`sf03_l4.rs` stale・speculative・equivalence |
| SF-ACT-04 Configured→Prepared→Published→Active 証拠分離・Last-good/EngineStub false positives 無し | §2.2 の状態機械・receipt 分離・`last_good_generation`・`EngineStub` 非証拠（型レベル分離） | slice1 + slice5 session 接続 | `sf03_l1.rs` activation 証拠、`sf03_session_engine.rs` session e2e |

## 8. 実装順序・検証計画・fixture 一覧

### 8.1 現状（main 統合済み）

- SF-03 slice1〜5: lowering/SourceMap/activation/fault/plugin host/
  NoRebuild/session 接続 — headless 実装済み。
- TRACE-C3: `ControlTrace`/`ControlTraceKey`/`ObservationFreshness`
  — project 層 + RT adapter（PC-C3-05）+ ControlSession 配線。
- SF-04: probe adapter→ControlSession→ControlTrace — headless 配線済み。
- SF-05: signal_flow/relation_ui headless — hierarchy 投影 +
  visual-only 不変 + 状態表示。
- PC-C6: REL/TR oracle headless fixture。

### 8.2 残存実装の順序（提案）

1. **consumer 横断の同一 Revision 検証強化**（headless）:
   probe/control/PCM/undo/save が同じ `ProjectRevision`+generation
   を指すことを cross-consumer fixture で固定（既存 `sf03_l*`・
   `sf04_*`・`sf05`・pc-c6 の組合せ + 不足分の追加）。
2. **SF-ACT-06 残件**: automation→`TimedParamEvent` lowering の
   engine 配線、Fast Control の active-generation gate
   （Blocked/有界 Queue）、GUI fast-control — headless 契約優先。
3. **SF-ACT-05 実機部分**: RT latency/underrun・実 device epoch
   restart・queue-full/partial stale command の実機検証 —
   **NotRun**（実機タスク）。
4. **SF-ACT-07 policy 決定**: Partial Preview vs Export
   fail-closed の mode 別 policy — 現行両 mode fail-closed 維持、
   policy 決定は後続 Issue。
5. **SF-ACT-08**: Ruviform RT/render/AudioLens 側の受入証拠 —
   upstream/別 consumer、**NotRun**（各 gate）。
6. **SF-ACT-09**: device config-only change の compile 判定・
   Golden & performance baselines — 設計変更を伴うため別 slice。
7. **実 OS/device/VST3・GUI scenario**（GUI-SF-01/GUI-PC-02）:
   **NotRun**（#114 gate）。

### 8.3 検証計画（headless 優先）

| 段 | 検証 | 手段 |
| --- | --- | --- |
| 契約 | `RoutingBuildKey`/`SourceMap`/`Receipt`/`Reconcile` の型・決定性 | `mise run test`（`routing_build`/`sf03_*`） |
| lowering | golden PCM・順序・tap・mute/solo・sidechain・Missing Source | `sf03_l1.rs`/`sf03_l2.rs` |
| activation | publish≠Active・frames=0・last-good・stale 拒否・equivalence | `sf03_l1.rs`/`sf03_l4.rs`/`sf03_session_engine.rs` |
| fault | 18 分類 mint・reconcile drift・retry fresh | `sf03_l2.rs`/`sf03_l4.rs` |
| plugin host | connect 順序・世代連動 teardown・fault 写像 | `sf03_l3.rs` |
| probe/trace | generation 照合・segment cut・freshness・stale reject | `sf04_*.rs`・pc-c3 fixtures |
| consumer 同一 Revision | probe/control/render/undo/save の相関キー一致 | cross-consumer fixture（§8.2-1） |
| oracle | REL/TR 独立照合 | pc-c6 `oracle_rel_tr` fixture |

### 8.4 fixture 一覧

**headless（実行可能）**:
- `crates/synvelis-player/tests/sf03_l1.rs`（lowering golden/
  SourceMap/activation SF-ACT-01〜04）
- `crates/synvelis-player/tests/sf03_l2.rs`（fault 18 分類/
  last-good/reconcile/offline gate）
- `crates/synvelis-player/tests/sf03_l3.rs`（plugin host 接続/
  世代連動 teardown）
- `crates/synvelis-player/tests/sf03_l4.rs`（NoRebuild/
  世代連動 control/SF-ACT-05 headless）
- `crates/synvelis-app/tests/`（session→routing e2e、sf05
  signal_flow）
- `crates/synvelis-workspace/tests/`（sf04_s2 probe trace、
  sf_05 projection）
- `crates/synvelis-project/tests/`（routing_build/trace/pc-c6
  oracle）

**NotRun（headless では検証不能）**:
- 実 OS device/VST3/L1 plugin の実機検証
- RT latency/underrun・実 device epoch restart の実測
- AudioLens f64/native PCM/analysis/RT 非回帰（実機）
- GUI scenario（GUI-SF-01/GUI-PC-02、#114 gate）
- SF-ACT-07 preview policy 分岐（policy 未規定）

**Unsupported 候補**: 現時点で確認されたものなし。Ruviform generic
の不足が判明した場合は Synvelis 側で恒久的な重複実装をせず、
基盤 Issue（ruviform）へ起票する — 本書はその運用を記録する。

## 9. Ruviform 不足時の扱い

- Ruviform generic（Typed Graph validator/compiler/GraphRuntime/
  RtRuntime/Slot/PDC/plugin host/probe broker）の不足が判明した
  場合、Synvelis 側で恒久的な重複実装・workaround を作らない。
- 一時的な回避策には理由・関連 Issue・解消条件を記録し、基盤
  Issue（`scarecrow173/ruviform`）へ起票する。現時点で blocking
  な不足は未検出（probe rebind #308・clock alignment #309・
  typed graph/RT API は pin `0f5b5737` で利用可能）。

## 10. Verification Notes（設計の検証）

- 本書は docs のみ。検証は `doc_status`/`doc_maintenance` lint +
  `mise run fmt-check` + review-protocol。
- 実装検証は §8.3 の headless fixture が担う（既存 `sf03_l*`・
  `sf04_*`・`sf05`・pc-c6 + cross-consumer 追加分）。
- 実機/GUI は NotRun として §8.4 に分離記録。
