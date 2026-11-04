---
id: "TSPEC-034SF01Oq1tUv6wX3yZaBcEfG"
type: "test-spec"
status: "approved"
title: "SF-01 Semantic Routing Model 検証仕様"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source: []
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
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/designs/sf-01-semantic-routing-model-design.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/designs/sf-01-semantic-routing-model-design.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-01 検証仕様(`crates/synvelis-project/tests/routing_sf01.rs`)

## Feature: Master exactly-one(SF-MODEL-01)

- Rule: `Project::new` の `routing` は Master Bus×1・その Strip×1・
  Chain×1・Main Input/Output×1 を持ち、`validate_references` を通る。
  - Example: `RoutingModel::default()` で buses が master のみ、
    `kind==Master` の bus が厳密 1 件。
- Rule: `AddBus` の `kind: Master` は拒否、Master Bus の
  `RemoveBus` は拒否。
  - Example: `AddBus{kind:Master}` → `Error::Validation`;
    `RemoveBus{master}` → `Error::Validation`。
- Rule: Master Insert(外部 `insert_count` 由来 FX)は persisted
  processor として存在しない。
  - Example: 全 processors map が migration/new project で空。

## Feature: 永続・ID・Undo(SF-MODEL-02)

- Rule: Bus/Strip/Processor/Chain/Port/Connection/Send は
  `Command`→`apply`→`save`→`load` で同一 content を復元。
  - Example: track strip + group bus + send + sidechain + processor
    chain 構成の save/reload で `Project` 等値。
- Rule: Undo/Redo は revision を進め、routing 内容を復元する。
  - Example: `AddSend`→undo で sends/connections が消え、redo で
    同一 ID で復元。
- Rule: 新規 Stable ID は `max_issued_id_raw` に含まれ、
  `id_watermark` が全 live ID を超える。
  - Example: op 発行 ID 群 < save 後 `id_watermark`、load が受理。
- Rule: reserved well-known 領域は generator 発行と衝突しない。
  - Example: seed=1 の generator 発行 TrackId/BusId が master
    entities(well-known raw)と別値。

## Feature: 参照整合・単一正本(SF-MODEL-03)

- Rule: SendConfig と `kind=Send` Connection は厳密 1:1。
  - Example: `AddSend` で conn+config 同時生成、`RemoveSend`/
    `RemoveConnection` で両方除去、`AddConnection{kind:Send}` は
    op validate 拒否、手作り孤立 send/config は load 拒否。
- Rule: 参照中 Track/Bus/Processor の削除は oracle で拒否、参照除去を
  含む `Composite` では受理。
  - Example: conn 残存の `RemoveTrack` → `Error::Validation`;
    `[RemoveConnection, RemoveTrack]` → 受理。
- Rule: 重複 (source,destination) connection・重複 processor・不正
  channel map・方向/domain/owner 違反は oracle で拒否。
  - Example: 同一 endpoint の 2 本目 Main conn は apply 拒否。
- Rule: strip graph の cycle は拒否(feedback cycle 全拒否)。
  - Example: bus A out→bus B in と bus B out→bus A in の同時存在を
    apply で拒否。

## Feature: Unresolved 保持・型分離(SF-MODEL-04)

- Rule: `ProcessorBinding::Unresolved` / `PortBinding::Unresolved` は
  そのまま永続化・reload で保持、別 port への自動付替なし。
  - Example: unresolved processor+ports を save/reload → binding
    等値。
- Rule: routing semantic ID は `TrackId` 等の既存 ID 空間や
  `ParameterTargetId` と型が分離(compile 時型・tagged serde)。
  - Example: `"bus-"` tag を `"track-"` field に食わせると
    deserialize エラー。

## Feature: v5→v6 Migration(SF-MODEL-05)

- Rule: v5 snapshot(schema_version=5・routing 無し)は read-only
  parse→legacy Clip→Master 相当投影→v6 として load。
  - Example: 2 track fixture → master + track strip×2 + Main conn×2、
    `live_revision = saved_rev + 1`。
- Rule: 同一 v5 bytes の再投影は決定的(同一 migration 結果)。
  - Example: 同一 fixture を 2 回 load → `Project` 等値(ID 含む)。
- Rule: 旧 v5 snapshot bytes/HEAD は不変で、次 save は新名を
  publish。
  - Example: 旧 `r5.json` bytes が load 後も同一、次 save は `r6.json`
    (旧名を上書きしない)。
- Rule: v5 body に `routing` field を持つ偽造入力は拒否。
  - Example: routing 込みの "v5" envelope → `Error::Validation`。
- Rule: Migration は source/asset/midi/notes/control/recording の
  既存意味を保持、unsupported version は `UnsupportedSchema`。
  - Example: fixture の clips/sources/tempo が load 後も同一内容。

## Feature: Golden・非回帰(SF-MODEL-05/06/07)

- Rule: `lower()` 出力は routing op 適用前後で不変(legacy
  compatibility path 保持、model 起点 lowering は SF-03)。
  - Example: `AddTrackStrip`/`AddBus`/`AddSend` 後の `lower()` が
    byte-identical(`GraphSpec`・playbacks)。
- Rule: Recording/Automation/Import の既存 test が非回帰。
  - Example: workspace `mise run test` 全 PASS。
- Rule: `RemoveParameterTarget` は send `gain_target` が参照する
  target を拒否。
  - Example: gain_target 使用中の target 削除 → 拒否、send 削除後は
    受理。

## Out of scope(別 Issue)

- Mute/Solo DAW 動作、RoutingTransaction Temp Ref、異 Layout
  channel map 承認、plugin capability 実検証、ControlOwner/
  AutomationTimeScope 分離、model 起点 lowering。
