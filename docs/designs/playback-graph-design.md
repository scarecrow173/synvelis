---
id: "DESIGN-034X6WwOONG7H4Rr2nHV2z"
type: "design"
status: "approved"
title: "Playback / Mixer / Generic Execution Graph Design — typed port, PDC, plan activation, owner separation"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/18"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/playback-graph.md"
  implemented-by: []
  depends-on:
    - "docs/specs/playback-graph.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/playback-graph.md"
  derived-by:
    - "docs/plans/playback-graph.md"
    - "docs/test-specs/playback-graph.md"
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/playback-graph.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  defers: []
  deferred-by: []
---

# Playback / Mixer / Generic Execution Graph Design

## Context

Synvelis は Ruviform を共通基盤とする DAW であり、Issue #18 は
Playback / Mixer / Generic Execution Graph の Typed Port・PDC・
Plan Activation の設計契約を要求する。上流 baseline
`playback-mixer-generic-execution-graph-architecture.md`（v0.4）
が 12 原則と EX-001〜020 を定義済みで、SF-03 map 設計が
semantic→runtime 写像・activation receipt・fault 分類を
headless 実装として固定済みである。本設計はそれらを統合し、
#104〜#109 の実装 Owner が参照する正本とする。

Owner 分離の核心: Synvelis が DAW 意味論の authoritative owner、
Ruviform が再利用 generic primitive の owner。DAW 固有型
（Track/Bus/Send/Clip/Automation）が generic runtime graph に
逆流してはならない。

## Scope

### In Scope

- Typed Port / Channel / Format 契約
- Generic Graph / Plan の責務境界
- Semantic lowering（Track/Bus/Send/Automation → generic graph）
- PDC 責務分割と compensation 契約
- Plan 世代状態機械と receipt
- ProjectController CAS / speculative plan 規則
- 単一 RT process の stateful ownership / retirement / recompile
- Structural vs Fast Control 分類
- Playback / prefetch / underrun / seek / loop / MIDI / offline /
  export / capture-monitor 分離
- Source mapping / probe evidence
- Failure taxonomy / recovery / last-good
- EX-001〜020 の設計写像

### Out of Scope

- 実装コード・Cargo・CI・Ruviform 変更
- GUI・実 device/plugin 受入
- 新規の generic graph primitive の Ruviform 側設計（不足は
  upstream Issue へ）

## Components and Responsibilities

| 層 | Owner | 責務 | 持たないもの |
| --- | --- | --- | --- |
| Semantic model | `synvelis-project` | RoutingModel・Track/Bus/Send/Clip/Automation・ProjectRevision・CAS・RoutingBuildKey・RoutingSourceMap・receipt/fault 型 | runtime graph 実行 |
| Lowering / activation | `synvelis-player` | `lower_semantic`・graph spec・playback planning・publish・reconcile・last-good ledger・processor host boundary | DAW 意味論の第二権威、PDC 機構の独自実装 |
| Render / export | `synvelis-render` | RenderSpec・Preflight・offline render job・staged WAV output・reopen verify | live playback との instance 共有 |
| Generic graph / RT / PDC / plugin | Ruviform | Typed Graph validator/compiler・GraphRuntime/RtRuntime・Slot/PDC・probe broker・plugin host/compiler | Track/Bus/Send/Clip 等の DAW 型 |
| Consumer（probe/control/UI） | workspace/session | read-only evidence・generation 照合・表示 | 独自 active 判定 |

依存方向は `synvelis-* → ruviform-*` の一方向。逆方向に DAW 型を
置かない。

## Data and Control Flow

### 1. Semantic → Lowering → Publish → Active

```
Project (semantic, revision R)
  → ProjectController.apply CAS commit (expected_revision)
  → committed snapshot から RoutingBuildKey / RoutingSourceMap 再計算
  → synvelis-player: lower_semantic → generic graph spec (Prepared)
  → Ruviform: validate/compile → publish (Published receipt)
  → RT process: frames>0 で active_generation 観測 → Active receipt
  → Observed (probe/control/PCM が generation で照合)
```

### 2. Speculative plan の禁止経路

```
gesture 候補 snapshot → semantic validation + lowering 予備検査
  → NG: 候補を破棄（publish しない）
  → OK: CAS commit → commit 成功後のみ正式 plan を再照合・publish
```

commit 前に作られた speculative compiled plan は publish されない。
commit 後に capability/format/clock を再照合し、不一致は publish
0 で Blocked/Failed receipt（last-good 維持）。

### 3. Structural vs Fast Control

- **Fast control**（fader/send gain/automation 値）: topology 不変、
  generation 不変の control channel で配送。dispatch ledger は
  plan generation を記録。
- **Structural**（routing/insert/plugin 構成/port 変更）: 非 RT で
  recompile → 新 generation を prepare/publish。再生中でも RT
  thread を block しない。pending 世代は次 publish で置換、
  last-good は RT 観測まで維持。

### 4. PDC / latency

- semantic lowering は PDC を直接符号化しない。Realtime/Offline は
  同一 `lower_semantic` を通り、topology（node/port/connection
  順序・tap 位置・sidechain 分離）が同一であることを契約とする。
- PDC 実効値（latency 報告・realignment 精度）は Ruviform RT
  contract と synvelis-render の責務。
- plugin latency 変更は capability drift として検出し、非 RT
  recompile を要求する（EX-008）。

## Typed Port / Channel 契約

- 全 edge は `PortRef`（format + channel layout + role）を持つ。
- `SemanticPort.channels` と engine channel の不一致は lowering
  error（deterministic 失敗）。
- `ExplicitChannelMap` は推論 upmix/downmix せず明示 map のみ。
- port 解決失敗は `PortUnresolved`、endpoint 未解決は lowering
  error、unsupported mapping は型付き fault。
- cycle（zero-delay feedback を含む）は compile/validation で拒否。

## Plan 世代状態機械

```
Configured{rev} → Validated{rev} → Prepared{rev,planned_generation}
    → Published{rev,gen} → Active{rev,gen,epoch}
         ↘ Blocked/Failed{fault,last_good_generation}
         ↘ Retired
```

- `Published` は非 RT publish 成功。`Active` は RT processing で
  frames>0 が expected active generation を観測した場合のみ mint。
- `last_good_generation` は最後に RT 観測された Active 世代。
- pending 世代（Published だが未 Active）は経過時間や停止中
  frames=0 では成功/失敗を断定しない。
- consumer は `observed_active_generation` のみを active 証拠とする。

## Publish 時再検証と Reconcile

`publish` は live project から `RoutingBuildKey` を再計算し、
project_id/revision・routing fingerprint・port_capability_generation
・source map/build key/generation 整合・generation 単調性・queue
容量を検証する。不一致は publish 0 で receipt。

`reconcile` は最後に publish された plan の key/source map を
live committed snapshot・RT counter と再照合し、drift を
`RoutingReconcileMismatch` として記録。優先順位:
`BadSourceMap` → `StaleProjectSnapshot` → `SpeculativeMismatch` →
`PluginCapabilityChanged` → `UnsupportedFormatOrTimeMap` →
`DeviceEpochChanged` → `RetireOverflow` → `EventStale` →
`ActivationNotObserved`。

## Failure Taxonomy / Recovery

`RoutingFault` 18 分類（Issue #106 §5 を正本として採用）:
`StaleProjectSnapshot` / `SpeculativeMismatch` /
`PluginCapabilityChanged` / `MissingSource` / `PluginUnavailable` /
`PortUnresolved` / `UnsupportedFormatOrTimeMap` / `InvalidRouting` /
`Cycle` / `CompileResourceExceeded` / `PublishQueueFull` /
`ActivationNotObserved` / `RetireOverflow` / `DeviceEpochChanged` /
`Superseded` / `BadSourceMap` / `EventStale` /
`OfflineCapabilityMismatch`。

Issue #18 が要求する 13 の失敗状況との対応:

| Issue #18 失敗状況 | fault/機構 |
| --- | --- |
| stale revision | `StaleProjectSnapshot` / CAS reject |
| capability drift | `PluginCapabilityChanged` |
| invalid graph/port/channel | `InvalidRouting` / `PortUnresolved` / lowering error |
| cycle | `Cycle` |
| missing source/plugin | `MissingSource` / `PluginUnavailable` |
| queue full | `PublishQueueFull` |
| pending activation | `Published` 継続（fault ではなく状態） |
| retirement overflow | `RetireOverflow` |
| latency change | `PluginCapabilityChanged` → 非 RT recompile |
| unsupported mapping | `UnsupportedFormatOrTimeMap` |
| prefetch underrun | `SkippedPlayback`/underrun 型（typed、fake silence 禁止） |
| device/clock epoch change | `DeviceEpochChanged` |

全 mint 経路は決定的で、未分類 fallback を作らない。fault 時は
last-good graph・configured project・receipt を保持し、GUI/Project
保存と RT 実行世代の相違を観測可能にする。

## Playback / Media / Offline / Export / Capture 分離

- **Playback**: 決定的 headless driver（`HeadlessDriver`）。
  prefetch miss は明示 underrun、EOF/seek/loop は決定的。
- **MIDI**: seek/loop 後の note-off 残り 0（panic/chase 契約）。
  `MidiPlaybackPlan`→`TransportPlan<EventKind>` lowering は
  TIM-02 契約に従う。
- **Offline render**: `PreparedRender` の孤立 GraphRuntime/RtRuntime
  で実行し、live playback の processor/plugin instance を共有しない。
- **Master/stem export**: 分離 job。stateful master render は逐次
  execution を維持。実 WAV は Finalize→別 Reader Reopen で検証。
- **Capture/monitor/playback**: recording raw capture・monitor・
  playback・PDC を独立経路として分離（X/L1/L2 契約の延長）。

## Probe / Source Map / Consumer 同一 Revision

- `SemanticProbePoint`→`resolve_probe_point` が publish 済み
  SourceMap で解決。binding は generation 付き、publish 時
  `RebindReceipt` で segment cut（自動継続しない）。
- stale record は `StaleGeneration` で trace 記録拒否。
- control/PCM/render/undo/save は `ProjectRevision`+plan
  `generation` を相関キーとし、独自の active 判定を持たない。
- source map は generation-scoped で publish 時に検証され、plan
  swap/plugin reload/port reorder/generation 遷移で無効化される。

## Non-Functional Constraints

- RT thread は allocation-free・blocking-free。structural recompile・
  publish・retire は非 RT。
- 決定性: 同一 semantic+build key は同一 lowering 結果。
- fail-closed: 未対応・不整合・欠落を fake silence/bypass で
  繕わない。
- 証拠の誠実さ: headless Pass を実機/GUI/plugin 受入に繰り上げない。

## Risks and Trade-offs

| Risk | Mitigation |
| --- | --- |
| Ruviform generic typed graph が arbitrary-port DAG を未完 | pin `0f5b5737` の API 範囲で記述し、不足は upstream Issue（#184/#308/#309）へ。Synvelis で恒久コピーしない |
| headless 証拠を実機受入と誤読 | capability 別 Pass/Fail/Unsupported/NotRun を evidence ledger で強制 |
| DAW 型の generic runtime 逆流 | Owner 分離表と依存方向一方向を設計・review で固定 |
| publish 成功を active と誤認 | Published/Active receipt 分離・frames>0 RT 観測を必須化 |
| consumer ごとの active 判定分岐 | generation 相関キー + observed_active_generation のみを証拠化 |

## EX-001〜020 設計写像

| EX | 設計上の担保点 | 主担当層 | 既存証拠（headless） |
| --- | --- | --- | --- |
| EX-001 source→output 決定的 PCM | `lower_semantic` 決定的順序 + ClipRenderer decode-to-memory | player/render | `sf03_l1` golden PCM、`l1_e2e` |
| EX-002 main+send fan-out 一回実行 | fan-out edge 共有・source node 単一実行 | lowering | `sf03_l1` send tap |
| EX-003 branch→return 決定的 mix | BTreeMap 順 lowering + 固定 sum 順 | lowering | `sf03_l1` 順序 |
| EX-004 aux sidechain 分離 | `Sidechain` edge→processor sidechain in + capability 検証 | lowering | `sf03_l1`/`l3` |
| EX-005 channel layout 不一致 | `SemanticPort.channels`≠engine→lowering error / `ExplicitChannelMap` | lowering | `sf03_l2` fault |
| EX-006 zero-delay feedback 拒否 | `Cycle` fault at compile/validation | ruviform+lowering | `sf03_l2` |
| EX-007 128/512 frame・384 PDC | Ruviform Slot/PDC + render realign、topology 同一性 | ruviform/render | `sf03_render`、tim04、render fixtures |
| EX-008 latency 変更→非RT recompile | `PluginCapabilityChanged`→recompile | player | `sf03_l4` |
| EX-009 再生中 structural 変更 | 非 RT recompile→publish、RT block なし | player | `sf03_l4` rapid publish |
| EX-010 stateful processor 単一 RT | 単一 RT process ownership・lease teardown | ruviform/player | `sf03_l3` lifecycle |
| EX-011 retirement queue 飽和 | bounded queue・`RetireOverflow` fault | ruviform/player | `sf03_l2` queue full |
| EX-012 fader/send automation | fast control・topology 不変 | player | `sf03_l6` automation |
| EX-013 seek/loop MIDI note-off | panic/chase・`TransportPlan` lowering | player/ruviform | `tim02_*`/`tim04_*` |
| EX-014 prefetch miss→underrun | typed underrun・fake silence 禁止 | player | `l1_e2e` underrun |
| EX-015 capture/monitor/playback 分離 | 独立経路・PDC 独立 | recording/player | `rcd_04_rt_comp_pcm` 等 |
| EX-016 live/offline 同値 | 同一 lowering・`OfflineCapabilityMismatch` gate | player/render | `sf03_l2`/`sf03_render` |
| EX-017 非決定 plugin 非保証 | capability で保証範囲を明示 | contract | 設計上の非保証明記 |
| EX-018 master/stem export 分離 | 分離 job・逐次 stateful render | render | `l2_e2e`/`x_e2e` |
| EX-019 2 地点 probe evidence | SourceMap 解決・generation binding・segment cut | player/workspace | `sf04_*` probe trace |
| EX-020 revision 相違記録 | ProjectRevision vs generation/epoch を別 receipt | project/player | `sf03_l1`/`l4` |

## Deferred Design Concerns

- 実 device/plugin/GUI gate の実測 | headless 設計のみ | #114・EXT-L3・ruviform#294 | 実装 Owner が headless を実機受入と誤読
- SF-ACT-07 preview/export policy | policy 未規定 | 後続 Issue | 失敗時挙動の分岐
- Ruviform arbitrary-port DAG 完成度 | pin 依存 | ruviform#184/#308/#309 | upstream 完成待ちで Synvelis 側 workaround の誘発

## Verification Notes

- [ ] `mise run fmt-check`（docs/設計スライス）
- [ ] `doc_status.js lint`（spec/design/plan/task/test-spec/impl）
- [ ] `doc_maintenance.js index apply`（触った dir の index 再生成）
- [ ] `audit_impl_record.js --json`
- [ ] `audit_docs.js`（該当時）
- [ ] `route_graph.js --focus playback-graph --json`
- [ ] `review-protocol`（spec/design chain、blocker なしを確認）
- 実装検証は #104〜109・Player/Render Owner の責務。本書は docs のみ。
