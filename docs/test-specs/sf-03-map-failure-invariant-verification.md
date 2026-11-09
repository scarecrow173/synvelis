---
id: "TSPEC-034UxcTb2ra5AlekrxQ8JG"
type: "test-spec"
status: "approved"
title: "SF-03 semantic→runtime 写像 — 世代証拠・失敗不変条件の検証仕様"
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
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
    - "docs/test-specs/sf-03-slice2-fault-reconcile.md"
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
    - "docs/test-specs/sf-03-slice5-session-engine-connect.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/100"
---

# SF-03 semantic→runtime 写像 — 世代証拠・失敗不変条件の検証仕様

対象: 本チェーンは docs のみの統合設計であり、実装は main 統合済みの
SF-03 slice1〜5 + TRACE-C3/SF-04/SF-05/PC-C6 を検証対象とする。
本 spec は各受入行の不変条件を headless fixture でどう証明するかを
固定する。実 OS/device/VST3・RT 実測・GUI は NotRun/Unsupported と
して分離する。

実行: `mise run test`（headless）。`mise run test-rt`・実機・GUI は
NotRun。

## Feature: SF-ACT-01 Golden PCM・SourceMap・Revision（MAP-R2/R3/R7）

| Rule | Example | 期待 |
| --- | --- | --- |
| Legacy direct→master 等価 | 同一 clip 配置で `lower` と `lower_semantic`+engine 駆動の出力 PCM を比較 | sample 完全一致（加算順序保存） |
| SourceMap↔revision | `lower_semantic` 結果の `source_map.project_revision`/`build_key.project_revision` | 渡した snapshot の revision と一致 |
| Missing Source 明示 | 未登録 source の clip | `SkippedPlayback::MissingSource` 記録 + `skipped_fault` 昇格で prepare 失敗。silent success でない |
| SemanticPort→PortRef | enabled `Main` connection | `source_map.connections[id] == Resolved(ConnectionId)`、`ports` が実 PortRef を指す |
| SourceMap 世代整合 | publish 時の `generation`/`build_key`/`project_id`/`project_revision` | 不整合は `BadSourceMap` で publish 0 |

## Feature: SF-ACT-02 順序・tap・PDC・RT/Offline 同意味（MAP-R3）

| Rule | Example | 期待 |
| --- | --- | --- |
| 決定的 lowering 順序 | strip chain・connections・playbacks を BTreeMap 順で lowering | 同一入力で node/port/connection ID・順序が決定的 |
| Pre/Post tap 分離 | `SendTap::PreFader`/`PostFader` の send | pre-out/post-out の別 PortRef へ解決、gain node 条件付き emit |
| Sidechain 分離 | strip post-out→processor sidechain in | sidechain edge が main path と分離して emit |
| Mute/Solo | `audible_strips` authority | mute/solo は edge prune + `NotLowered(MutedStrip)` 記録。processor 内に mute 状態を持たない |
| Bus/Return・Processor 順序 | insert chain 宣言順 | main-in→chain 順→fader/pan→post-out の信号順を保持 |
| RT/Offline 同一 lowering | `BuildMode::Realtime`/`Offline` で同一 `RoutingModel` を lower | node/port/connection topology が同一。offline は `OfflineCapabilityMismatch` gate のみ追加 |
| 未解決 mapping | `ProcessorBinding::Unresolved`・rate 不一致・channel map 非対応 | `Unavailable`/`Unsupported`/`NotLowered` または deterministic 失敗。fake silence でない |

## Feature: SF-ACT-03 Gesture CAS・speculative publish 0（MAP-R4/R5）

| Rule | Example | 期待 |
| --- | --- | --- |
| Candidate≠Committed | gesture 候補 snapshot での speculative 検証 | commit 前の予備検査のみ。speculative compiled plan は publish されない |
| CAS stale | `ProjectController.apply` に stale expected_revision | commit 前に reject。publish 側も `StaleProjectSnapshot`/`SpeculativeMismatch` で publish 0 |
| Plugin Cap drift | `port_capability_generation` が prepare 後に変化 | publish で `PluginCapabilityChanged`、publish 0、last-good 維持 |
| Temporary ID mismatch | source map/build key/generation の内部不整合 | `BadSourceMap` で publish 0 |
| 連続編集 | rapid R1/R2/R3 の連続 prepare→publish | 未 Active の pending 世代は次 publish で置換。`last_good_generation` は RT 観測まで後退しない |
| Superseded | 既 publish 世代以下の generation で publish 試行 | `Superseded` で拒否 |
| NoRebuild | semantic_eq が真で revision のみ異なる | `ExecutionEquivalence` を返し compile/generation 消費なし。新 Revision が Active になった偽 receipt を出さない |

## Feature: SF-ACT-04 世代証拠分離・Last-good・Pending（MAP-R1）

| Rule | Example | 期待 |
| --- | --- | --- |
| Configured→Validated→Prepared→Published→Active | 各段の receipt state | `GraphRuntime::publish` 成功は `Published` まで。`Active` は RT `process(frames>0)` が `active_generation` 一致を観測した時のみ mint |
| Publish success≠RT 駆動 | publish 成功後 RT 未駆動 | receipt は `Published` のまま。consumer は `observed_active_generation` のみを active 証拠とする |
| Pending 明示（停止中/frames=0） | RT 停止中または frames=0 で publish | receipt は `Published` のまま `PendingActivation` ledger が frames を累積。経過時間だけで成功/失敗を断定せず、overdue は reconcile `PendingActivation`/`ActivationNotObserved` mismatch で検出 |
| RT active delayed | publish 後に遅延して RT callback が新世代を観測 | その時点で `Active` に遷移。`last_good_generation` が前進 |
| Last-good 維持 | compile/publish/activation 失敗・reconcile drift・superseded | `last_good_generation` は後退しない。旧 plan が音声を維持 |
| EngineStub false positive 無し | `EngineStub`/非 RT 経路での activation 主張 | stub は receipt を生成できず `graph_builds` は build 計数のみ。consumer がこれを Active 証拠とみなさない |

## Feature: Plan publish 失敗不変条件（MAP-R5）

| Rule | Example | 期待 |
| --- | --- | --- |
| publish QueueFull | command/event channel 満杯で publish | `PublishQueueFull` で失敗、旧 plan 維持 |
| stale command/event | 旧 generation 宛の dispatch/event | RT 側 discard→counter 前進→reconcile `EventStale`。wrong-target 配送 0 |
| device restart/epoch | `RuntimeInstanceId`/`RuntimeClockEpochRef` 変化 | `DeviceEpochChanged`。旧 generation 無効、新 prepare が必要 |
| plugin missing/reload | plugin 不在・capability 変化・reattach | `PluginUnavailable`/`PluginCapabilityChanged` で fail-closed。世代連動 lease teardown・probe rebind segment cut |
| retire overflow | bounded retire channel 溢れ | `RetireOverflow` fault。clean success でない |
| reconcile drift | publish 済み plan の BuildKey/SourceMap を live snapshot と再照合 | 全不一致側面を `RoutingReconcileMismatch` に記録、優先順位で 1 fault を receipt（Blocked）。旧 plan 維持 |
| retry fresh prepare | publish/activation 失敗後の retry | 常に新 BuildKey/SourceMap/generation から開始。失敗した `PreparedRouting`・旧 generation・旧 source map を再利用しない |
| Missing source/port/plugin | clip source 未解決・`SemanticPort` 未解決・plugin instance 未供給 | `MissingSource`/`PortUnresolved`/`PluginUnavailable`。bypass・fake silence で誤魔化さない |

## Feature: Consumer 同一 Revision 契約（MAP-R6）

| Rule | Example | 期待 |
| --- | --- | --- |
| Probe 世代照合 | `resolve_probe_point` + `SemanticProbeBinding` | publish 済み SourceMap で fail-closed 解決。stale binding は `StaleGeneration` で reject、trace 記録しない |
| Control 世代スコープ | `take_processor_control(generation, processor_id)` | 公開済み世代の control のみ one-shot 払い出し。旧世代への配送経路なし |
| ControlTrace freshness | `ControlTraceKey.graph_generation`/`control_plan_generation`/`clock_epoch`/`evaluation_position` | `ObservationFreshness` が不一致を `Stale`/`Unavailable` に分類、誤 Live を出さない |
| PCM render binding | `playback_bindings`/`RoutingPlaybackRef` | semantic content→runtime node/out port を publish 時に install。旧世代 playback は retire と共に解放 |
| UI evidence | SF-05 `SignalFlowRuntimeEvidence` | receipt/source_map/probe bindings の snapshot を read-only 消費。独自 active 判定を持たない |
| Undo/Save | undo/redo・save | 新 revision → 通常 rebuild/equivalence 判定。Save は semantic model のみ永続化（runtime ID を含まない） |

## NotRun / Unsupported

以下は headless では検証不能であり、本 spec の合格証拠に含めない。
成功と誤表示しない。

| 項目 | 理由 | 送り先 |
| --- | --- | --- |
| 実 OS device/VST3/L1 plugin の実機検証 | 実デバイス・OS 依存 | #114 gate / 実機タスク |
| RT latency/underrun・実 device epoch restart の実測 | RT スケジューラ・実デバイス依存 | SF-ACT-05 実機部分 |
| AudioLens f64/native PCM/analysis/RT 非回帰 | 実機 AudioLens 環境依存 | SF-ACT-08 / upstream Issue |
| GUI scenario（GUI-SF-01/GUI-PC-02） | GUI/OS 統合 | #114 gate |
| SF-ACT-07 Partial Preview vs Export policy 分岐 | policy 未規定 | 後続 policy 決定 Issue |
| SF-ACT-09 device config-only compile 判定・性能 baseline | 設計変更を伴う | 性能評価 slice |

Ruviform generic の不足が判明した場合は Synvelis 側で恒久的な
重複実装をせず、基盤 Issue（`scarecrow173/ruviform`）へ起票する。
現時点で blocking な不足は未検出（probe rebind #308・clock
alignment #309・typed graph/RT API は pin `0f5b5737` で利用可能）。

## Sources and Relations

- `docs/specs/sf-03-map-semantic-runtime-contract.md`（MAP-R1〜R8）
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`（§2 世代
  証拠・§5 CAS/publish・§6 失敗不変条件・§7 SF-ACT 対応・§8 検証計画）
- `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md`
  （authoritative owner 決定）
- 既存 test-spec: `sf-03-slice{1,2,4,5}-*`（本 spec はその上位統合
  検証として追加し、slice spec を supersede しない）
- 実装 fixture: `crates/synvelis-player/tests/sf03_l{1..4}.rs`、
  `crates/synvelis-app/tests/`、`crates/synvelis-workspace/tests/`、
  `crates/synvelis-project/tests/`（trace/probe/oracle）
