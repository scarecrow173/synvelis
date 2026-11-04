---
id: "SPEC-034SF01Km7pQr2sT9uVwXyZaB"
type: "spec"
status: "approved"
title: "SF-01 Semantic Routing Model — Bus/Send/Processor Chain/Stable Port IDs・Project Schema v6"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/4"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
    - "docs/designs/sf-01-semantic-routing-model-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/4"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-01-e2e-semantic-routing-model.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/ruviform/issues/184"
  defers: []
  deferred-by: []
---

# SF-01 Semantic Routing Model — Project 正本・Schema v6 Migration

## What Should Be Built

Issue #104「SF-01 確定詳細契約」(2026-11-04 合意) を実装する。Synvelis の
Project 集約に Semantic Routing Model を導入し、Bus / ChannelStrip /
ProcessorChain / Processor / SemanticPort / RoutingConnection / SendConfig
を stable typed ID で永続化する。`ProjectController::apply` が唯一の
Writer、`Project::validate_references()` と `check_id_watermark()` が
参照整合の最終 oracle。

現行 `Track { id, name, lane_ids }` は制作コンテナ、現行 `Project` には
永続 Bus/Strip/ProcessorChain/Send/Connection が存在しない。
`synvelis-player::lower_with_inserts` の引数 `insert_count` 由来 FX は
旧 Project に永続化されていないため、Migration で仮想 FX として
**作成しない**。

## Why

- Routing/Control チェーンの起点: CTL-01 #121 → SF-02 #105 → SF-03 #106 /
  CTL-02 #122 の前提となる Project 正本。
- Track(制作表示 Owner) と ChannelStrip(DSP/Fader/Pan/Insert/Send Owner)
  は別概念。`TrackId` と `StripId` は別 Stable ID。
- Runtime `ruviform_graph::{NodeId, PortId}` や Plugin Local Port
  identifier、Process slot、Node generation は Project semantic ID と
  別型であり、数値一致での自動付替は禁止。未解決は `Unresolved` を
  明示保持する。

## Acceptance Criteria(SF-MODEL 系、Issue #104 §5 が正本)

- **SF-MODEL-01**: Empty/New/Legacy v5 Project で Master が厳密 1 件、
  Track/Bus/Strip の責務分離、旧 Clip→Master 相当経路、Master Insert
  (外部 `insert_count` 由来) の非捏造。
- **SF-MODEL-02**: Track/Bus/Return/Send/Sidechain/Chain の
  Save→Reload、Undo→Redo、Typed ID / tagged serde / Map key↔id /
  ID watermark の整合。
- **SF-MODEL-03**: 同一 Bus/Strip の 2 重 DSP 生成なし、Send と
  Connection の厳密 1:1、無許可 Delete / Dangling / 重複 Processor /
  不正 Map は検証 Reject。
- **SF-MODEL-04**: Missing Plugin/Port は `Unresolved` 保持、無断の別
  Port reconnect なし、Graph Runtime ID 再採番と Project semantic ID は
  無関係。
- **SF-MODEL-05**: Legacy v5 migration の read-only 再実行決定性、旧
  Snapshot/HEAD 保護、既存 Lowering の legacy compatibility 保持と
  Golden 比較、Missing Source/Asset と opaque state 保持。
- **SF-MODEL-06**: Workspace layout/Probe toggle/Device change で
  Project DSP/Undo/Diff が不変(visual vs semantic hierarchy 分離)。
- **SF-MODEL-07**: 参照中 Track/Bus 削除は Reject、または参照除去を
  含む単一 Composite Transaction でのみ許可。Recording/Automation/
  Import/Save の非回帰。

## 型付きデータ契約(実型名・crate 配置は ADR/設計で最終化)

```rust
pub struct RoutingModel {
    pub master: BusId,               // exactly one logical master
    pub buses: BTreeMap<BusId, Bus>,
    pub strips: BTreeMap<StripId, ChannelStrip>,
    pub processors: BTreeMap<ProcessorId, Processor>,
    pub chains: BTreeMap<ChainId, ProcessorChain>,
    pub ports: BTreeMap<SemanticPortId, SemanticPort>,
    pub connections: BTreeMap<RoutingConnectionId, RoutingConnection>,
    pub sends: BTreeMap<SendId, SendConfig>,
}
pub enum StripOwner { Track(TrackId), Bus(BusId) }
pub enum BusKind { Group, Return, Master }
pub enum ConnectionKind { Main, Send(SendId), Sidechain }
pub struct RoutingConnection {
    pub id: RoutingConnectionId,
    pub source: SemanticPortId,
    pub destination: SemanticPortId,
    pub kind: ConnectionKind,
    pub enabled: bool,
    pub channel_map: Option<ExplicitChannelMap>,
}
pub struct SendConfig {
    pub id: SendId,
    pub connection: RoutingConnectionId,
    pub tap: SendTap,
    pub gain_target: Option<ParameterTargetId>,
}
pub enum SendTap { PreFader, PostFader }
```

- `RoutingConnection` を唯一の接続先正本。`SendConfig` は送出設定
  (tap / level binding) であり別送信先・別 Edge を持たない。
  `Send(id)` ↔ `SendConfig { id, connection }` は厳密 1:1 相互参照。
  孤立 Config / Send Edge / 二重参照を拒否。
- Master は論理 Project Bus で厳密 1 件。Audio Device Endpoint /
  Device ID ではない。
- `SendConfig.gain_target` は `Project::control` の
  `ParameterTargetId` を Authority とする参照のみ。第 2 の Persisted
  Gain 値を作らない(CTL-01 #121 / PC-C4 #101 と調整。現行
  `ControlScope` に Bus/Strip/Send scope は未導入のため SF-01 では
  存在検証のみ、owner mapping は CTL-01 の接続点として文書化)。

## Schema Migration(単一 Versioned Migration)

- 旧 v5 読込 → 元 Bytes/HEAD 固定 → read-only parse → Legacy
  Direct-to-Master 経路を表す Routing Projection 生成 → semantic/ID
  検証 → 新 Schema snapshot へ safe publish。旧 v5 Snapshot/HEAD/
  Asset/Source/opaque plugin bytes を上書きしない。
- 新 Schema 番号は既存 #17 / ALIB-02 #86 / VCS-01 #92 と共通の
  Versioned Reader / Migration Registry。独立した SF 専用
  Migrator/ProjectStore は作らない。厳密 schema version 検査を保持し、
  `#[serde(default)]` を空 Routing への完全 Migration と誤認しない。
- Migrator が新規生成する Bus/Strip/Port Stable IDs は元 Project の全
  Map + watermark と衝突しない明示 ID 割当規則。同一旧 Snapshot の
  再投影は決定的。Migration 後の正式保存は `id_watermark` を新 ID
  発行位置へ進める。
- 段階的移行中も Source/Track/Clip/Recording/Control の既存意味を保持。
  Plugin/Port Unavailable は `Unresolved` 報告。自動的に別 Plugin へ
  routing を付替えない。

## Scope Boundary(本 dispatch の範囲)

- **In scope**: RoutingModel 型・参照整合・Project Schema v6 組込・
  Migration 設計、`ProjectController` のコマンド/apply 経路への
  RoutingModel 操作統合(Undo / 1 CAS 契約)、SF-02 が必要とする
  Command 設計の接続点文書化。
- **Out of scope**: Mute/Solo の具体 DAW 動作、Routing Transaction
  (Temp Ref→Stable ID、独立 preflight/staging) は SF-02 #105。
  RoutingModel → GraphSpec lowering は SF-03 #106。`ControlOwner` /
  `AutomationTimeScope` の型分離と旧 `ControlScope` migration は
  CTL-01 #121。Host Parameter 既定値は CTL-02 #122。Plugin
  Capability/Binding 検証は PC-C4 #101。

## Non-Functional Requirements

- cargo コマンドは `mise run <task>` 経由。`checked_add`/`checked_sub`
  による時間演算のみ。
- Ruviform へ `TrackId`/`SendId`/`ProjectRevision` 等の Synvelis 型を
  導入しない。`GraphSpec`/`NodeId`/`PortId` は Project から再生成する
  Derived Execution IR であり正本ではない。
- UI Node 位置 / Visual Group / Probe Mode / Runtime Plan Generation は
  永続 DSP Project の管理対象外。
