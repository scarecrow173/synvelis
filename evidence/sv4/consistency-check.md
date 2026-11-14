# Evidence — synvelis#4 Signal Flow 設計正本: 既存 SF 系 spec/design との整合確認

`docs/designs/signal-flow-design-design.md` が既存の SF-01〜06
spec/design・関連設計と矛盾しないことを確認した記録。確認は
文書間の**意味論照合**であり、実装テストの実行結果ではない。

## 照合方法

- 対象: `docs/specs/sf-01..06-*.md`、
  `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`、
  `docs/designs/sf-05-slice{1,2}-*.md`、
  `docs/designs/playback-mixer-generic-execution-graph-architecture.md`、
  `docs/test-specs/sf-05-*`・`docs/test-specs/sf-06-*`。
- 方法: design が参照する型名・状態語彙・Case family・lane 概念を
  各正本 doc と `crates/` の実装語彙に照合。設計正本は詳細契約を
  **再掲せず参照**する方針のため、矛盾は「参照先との意味論の
  不一致」としてのみ評価した。

## 確認結果

### 型・状態語彙の一致（実装照合）

| design の記述 | 正本/実装 | 結果 |
| --- | --- | --- |
| `ProbeMode { Off, Monitor, Capture }` | `crates/synvelis-project/src/probe.rs:122` | 一致 |
| `ProbeResolveState` 11 variant（Requested/Unresolved/Unavailable/Unsupported/QueuedForBinding/PendingRtInstall/BoundActive/StaleGeneration/PartiallyRebound/Lost/Stopped） | `crates/synvelis-project/src/probe.rs:164` | 一致（design §D.3 を実装語彙に修正済み） |
| `RoutingEditError` 分類（StaleRevision/StalePortCapability/DuplicateOwnership/UnresolvedPort/UnresolvedReference/InvalidDomainOrDirection/UnsupportedChannelMap/FeedbackCycle 等） | `crates/synvelis-project/src/routing_tx.rs:332` | 一致 |
| `SignalFlowNodeRuntime`（Unobserved/Resolved/Unavailable/Unsupported/NotLowered/Stale） | `crates/synvelis-workspace/src/signal_flow_ui.rs:118` | 一致 |
| `SignalFlowEdgeRole { Main, Sidechain, Send, Chain }` | 同:189 | 一致 |
| `SignalFlowScope { Overview, Local, Expanded, History }` | 同:237 | 一致 |
| `SignalFlowLayer`（Audio 他、`#[non_exhaustive]`） | 同:262 | 一致 |
| `SignalFlowVisualGroup`（id/label/members/collapsed） | 同:423 | 一致 |
| `audible_strips`（Mute/Solo の可聴経路評価） | `crates/synvelis-project/src/routing.rs:661` | 一致 |
| `ProjectOp`（Routing transaction の入口） | `crates/synvelis-project/src/ops.rs:45` | 一致 |

### 契約意味論の一致（文書照合）

| 項目 | 正本 | design の扱い | 結果 |
| --- | --- | --- | --- |
| Semantic/Visual Hierarchy 分離 | sf-05 slice1 spec/design、SF-MODEL-06 | §A.1 で `SignalFlowVisualState.groups` を ViewState と明記 | 一致 |
| Node 分類 Container/Processor/Endpoint | sf-05 slice1、Issue #4 | §A.2 で semantic entity との対応を表形式で固定 | 一致 |
| scope Overview/Local/Expanded/History | sf-05 slice1 | §A.3、History は `coverage=Unsupported` と明記 | 一致 |
| Port Drag lifecycle（fixation→preview→confirm→1 CAS/1 Undo） | sf-05 slice2 spec（SF-UX-03）、SF-TX-01 | §C.2 で 5 段階を転記 | 一致 |
| ConnectionKind Main/Send/Sidechain + SendConfig 1:1 | sf-01 spec（SF-VALID-02）、sf-02 | §B.2、同一 pair 併存を dedup しない旨を明記 | 一致 |
| 接続検証 → `RoutingEditError` 分類 | sf-02 spec | §B.3 で検証対象→分類の対応表 | 一致 |
| 世代証拠（Configured/Prepared/Published/Active/Observed/Last-good） | sf-03-map spec/design | §E.1 で 6 層の名前空間を表形式で固定 | 一致 |
| `RoutingSourceMap` 単一権威・非正本・generation-scoped | sf-03-map ADR/spec | §E.2 | 一致 |
| Probe 状態語彙と `BoundActive` のみ RT 観測済み | sf-04 spec（SF-PROBE-*） | §D.3、実装 enum に整合 | 一致 |
| Probe ON/OFF が Bypass/Routing を変更しない | sf-04 spec（SF-PROBE-02） | §D.4 不変条件 | 一致 |
| scenario ID が #109 Case registry を再定義しない | sf-06 spec | §E.4 で「設計追跡子」と明記 | 一致 |

### 発見した差異と対応

1. **Probe 状態語彙の表記揺れ**: Issue #4 本文は `Unbound`/`Partial`/
   `Unknown` 等の概念名を使用するが、実装 enum は
   `PartiallyRebound`/`StaleGeneration`/`Stopped` 等。design §D.3 で
   実装語彙を正本とし、概念名との対応を注記して解消。
2. **`UnresolvedReference` variant**: `RoutingEditError` に design
   §B.3 の表で個別行を設けなかった variant が存在。表は「検証対象
   → 分類」の対応であり網羅を意図しないため矛盾ではないが、
   Port 解決失敗の一分類として `UnresolvedPort` と併記される
   べきことを確認（P3 指摘・対応不要と判断）。
3. **既存 broken relation**: `docs/specs/sf-03-map-semantic-runtime-
   contract.md` の `defers` が非ファイル target を指し、
   doc-driven-dev-graph router が repository 全体で blocked を
   返す。本タスク範囲外の既存不備であり、本 design の relations は
   全て解決可能なファイル/URL のみを使用。

## 結論

設計正本は既存 SF 系正本と意味論的に矛盾しない。詳細契約は各正本
doc を authoritative とする参照構造を維持し、View 層の対応関係と
受入追跡のみを本書に固定した。
