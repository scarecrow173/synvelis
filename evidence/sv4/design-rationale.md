# Evidence — synvelis#4 Signal Flow 設計正本: Decision Log 根拠・代替比較

設計正本 `docs/designs/signal-flow-design-design.md` の Decision Log
（D1〜D15）について、採用/非採用/保留の根拠と検討した代替案を記録する。
本ファイルは設計判断の**根拠記録**であり、実装検証の結果ではない。

## 決定の出所

| 出所 | 内容 |
| --- | --- |
| Issue #4 本文（2026-11-04〜10 合意） | Semantic/Visual Hierarchy 分離・直接編集 UX・標準 Probe・Ruviform/Synvelis Ownership・全 cycle 初期 Reject・`ExplicitChannelMap`+consent・Publish/Active/Observed 分離 |
| SF-01〜06 spec/design | `RoutingModel`/`SemanticPort`/`RoutingConnection`/`SendConfig`・`RoutingEditError`・`RoutingSourceMap`/`RoutingBuildKey`/`RoutingActivationState`・`ProbeResolveState`/`SemanticProbePoint`・`signal_flow_graph`/`SignalFlowView`/`SignalFlowVisualState`/`RoutingGesture` |
| 実装語彙照合 | `crates/synvelis-project/src/probe.rs`（`ProbeMode`/`ProbeResolveState`）、`crates/synvelis-project/src/routing_tx.rs`（`RoutingEditError`）、`crates/synvelis-workspace/src/signal_flow_ui.rs`（`SignalFlowNodeRuntime`/`SignalFlowEdgeRole`/`SignalFlowScope`/`SignalFlowLayer`/`SignalFlowVisualGroup`）、`crates/synvelis-project/src/routing.rs`（`audible_strips`）を読み取り照合（変更なし） |

## 主要決定の代替比較

### D1 View = read-only 投影 + ViewState（第二 domain model を作らない）

- **採用**: `signal_flow_graph` が `RoutingModel` を投影し、
  `SignalFlowVisualState` が `ViewInstanceId` 単位の非 DSP 状態を
  保持する構成（sf-05 slice1 実装済み）を正本化。
- **代替（非採用）**: Signal Flow 専用の第二 Routing グラフを
  View が保持し同期する案 — 第二正本となり Mixer/Undo/Project と
  乖離する危険があるため棄却。Issue #4「表示と実行の分離」に反する。

### D3 Port Drag = Preview→confirm で 1 CAS/1 Undo

- **採用**: `RoutingGesture` の fixation→hover(dry-run)→drop→
  confirm→`ProjectOp::Routing` の lifecycle（sf-05 slice2/SF-UX-03）。
- **代替（非採用）**: drag 中に逐次 commit する案、hover で
  Project を変更する案 — preflight preview が Project を変更
  しない契約（SF-TX-01）と Undo 境界（1 gesture = 1 Undo）に反する。

### D6 Main/Send/Sidechain を kind 別 edge として区別

- **採用**: `ConnectionKind { Main, Send(SendId), Sidechain }` と
  `SendConfig` の厳密 1:1 参照を表示・編集の区別単位とする
  （SF-VALID-02）。同一 (source, destination) pair の併存を
  dedup せず kind ごとに区別する。
- **代替（非採用）**: pair 単位で 1 edge に集約し kind を属性表示
  する案 — Send/Sidechain/Main の編集対象が曖昧になり、Issue #4
  受入「Main と混同せず」に反する。

### D9 世代証拠の区別表示（Publish != Active を潰さない）

- **採用**: `ProjectRevision`（Configured）・`RoutingBuildKey`・
  plan generation（Prepared/Published）・`active_generation`
  （Active）・Observed を generation 付きで区別表示
  （sf-03-map・SF-UX-05・Issue #4「音声に関する説明の誠実さ」）。
- **代替（非採用）**: 「接続済み=反映済み」の単一 status 表示 —
  `Configured R_new / Active R_old` の区別を失い、UI クリック
  時点を DSP 適用済みとみなす誤りを招く（Issue #4 禁止事項）。

### D12 特殊 Feedback は初期一律 Reject

- **採用**: `RoutingEditError::FeedbackCycle` で全 cycle を拒否
  （遅延有無を問わない）。
- **代替（非採用）**: 遅延付き loop を許可する案 — Issue #4 で
  「Runtime Capability 検証と別明示合意」まで保留と合意済み。
  安全側拒否を維持。

## 保留事項（提案/決定待ち — 採用決定ではない）

| 事項 | 状態 | 判断が必要な時点 |
| --- | --- | --- |
| Visual Group の保存・共有粒度（Project 永続 vs ViewState のみ・複数 View 共有） | 保留 | Visual Group 高度 UI slice。現行は `ViewInstanceId` 単位 ViewState |
| 複数 Channel Mapping の UI 提供方式・既定 | 保留 | channel map 編集 UI slice。`ExplicitChannelMap`+consent が現行契約 |
| Graph Canvas 描画・ドラッグ閾値・Semantic Zoom 数値 | 保留 | 実 GUI slice（#108/#114） |

## 非採用（明示的棄却）

- D13 Inspect/Edit 大 Mode 切替の必須化 — Issue #4 が「初期仕様
  として必須にしない」と明記。操作入口の分離（click=Inspect、
  drag=編集 gesture、明示 Command=編集）で代替。
- D14 Graph Canvas 共通 UI の Ruviform 要求 — Issue #4「両
  Consumer 等から具体的な共通要求が発生した場合のみ」。現時点は
  framework-neutral 契約（ruviform#257）の範囲。
- D15 Probe PCM Capture を Recording Writer と同一機構化 —
  Lossy Probe PCM と Lossless/Fail-stop Recording を混同しない
  （Issue #4・SF-PROBE-05）。
