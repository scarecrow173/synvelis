---
id: "IMPL-034SF01Qr3wXmZyAbCdEfGhJ"
type: "impl"
status: "completed"
title: "SF-01 Semantic Routing Model — Bus/Send/Chain/Stable Port IDs・Schema v6"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/104"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/routing.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/routing_sf01.rs"
      - type: "file"
        path: "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
      - type: "file"
        path: "docs/adr/sf-01-routing-model-schema-v6-migration.md"
      - type: "file"
        path: "docs/designs/sf-01-semantic-routing-model-design.md"
      - type: "file"
        path: "docs/plans/sf-01-semantic-routing-model.md"
      - type: "file"
        path: "docs/test-specs/sf-01-e2e-semantic-routing-model.md"
      - type: "file"
        path: "docs/tasks/sf-01-task1-routing-model-schema-migration.md"
      - type: "file"
        path: "docs/impl/ir/sf-01-semantic-routing-model.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/midi_source.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-01-semantic-routing-model.md"
    - "docs/designs/sf-01-semantic-routing-model-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/105"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/104"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-01-e2e-semantic-routing-model.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-01 Semantic Routing Model 実装記録

## Summary

Issue #104(SF-01)。`synvelis-project` に永続化される Semantic Routing
Model(`Project::routing`)を導入し、Bus/ChannelStrip/ProcessorChain/
SemanticPort/RoutingConnection/SendConfig を Stable Semantic ID で
model 化した。Schema v5→v6 の単一 Versioned Migration 経路・
`ProjectController` の 1 CAS/1 Undo command 経路への ops 統合までを
実施。Runtime lowering・Mute/Solo DAW 動作・Temp Ref→Stable ID
transaction は SF-02/#105・SF-03/#106 の範囲として接続点のみ残す。

## Context

Issue #104「SF-01 確定詳細契約」(2026-11-04 合意)。Synvelis の
Project 集約に Semantic Routing Model を導入する slice で、
CTL-01 #121 → SF-02 #105 → SF-03 #106 / CTL-02 #122 の前提と
なる Project 正本を作る。現行 `Track { id, name, lane_ids }` は
制作コンテナであり、永続 Bus/Strip/ProcessorChain/Send/
Connection は存在しない。Track（制作表示 Owner）と
ChannelStrip（DSP/Fader/Pan/Insert/Send Owner）は別概念で、
`TrackId` と `StripId` は別 Stable ID。Runtime
`ruviform_graph::{NodeId, PortId}`・Plugin Local Port
identifier・Process slot・Node generation は Project semantic
ID と別型で、数値一致での自動付替は禁止、未解決は
`Unresolved` を明示保持する。正本は
`docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md`、
schema v6 migration の決定は
`docs/adr/sf-01-routing-model-schema-v6-migration.md`。
本 slice の範囲は RoutingModel 型・参照整合・Schema v6 組込・
Migration・`ProjectController` ops 統合まで。Mute/Solo DAW
動作・Temp Ref→Stable ID transaction（SF-02/#105）、
RoutingModel→GraphSpec lowering（SF-03/#106）、
`ControlOwner`/`AutomationTimeScope` 型分離（CTL-01/#121）、
Host Parameter 既定値（CTL-02/#122）、Plugin Capability/
Binding 検証（PC-C4 #101）は対象外。

## Implementation

### 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `id.rs` | `BusId`/`StripId`/`ProcessorId`/`ChainId`/`SemanticPortId`/`RoutingConnectionId`/`SendId` を `define_id!` で追加(tagged `"<kind>-<32hex>"`、runtime `NodeId`/`PortId` とは別型) |
| `routing.rs`(新規) | `RoutingModel`(master + 7 entity map)、`Bus`/`BusKind`/`ChannelStrip`/`StripOwner`/`ProcessorChain`/`Processor`/`ProcessorBinding`/`PluginRef`/`SemanticPort`/`PortOwner`/`PortDirection`/`PortRole`/`SignalDomain`/`PortBinding`/`RoutingConnection`/`ConnectionKind`/`ExplicitChannelMap`/`SendConfig`/`SendTap`。`Default` = well-known master bootstrap、`RoutingModel::validate(&Project)` が全不変条件の oracle、enabled edge の strip グラフ非循環検査 |
| `migration.rs`(新規) | `STEPS` ordered registry(`v→v+1`、#86/#92 と共用する単一経路)。`migrate_v5_routing`: v5 body を read-only 投影し、track 毎に strip+chain+main ports+master への `Main` connection を決定的 ID 割当で生成。`insert_count` 由来の仮想 processor は作らない。routing を既に持つ v5 body は拒否 |
| `project.rs` | `Project.routing`(serde default、v5 body の parse 用 — migration は別経路)、`validate_references` 末尾で `routing.validate(self)`、`max_issuable_id_raw`(旧 `max_issued_id_raw`)へ routing 7 map fold + reserved 値除外、`check_id_watermark` に `< WELL_KNOWN_BASE` 条件 |
| `store.rs` | `SCHEMA_VERSION` 5 → 6 |
| `controller.rs` | `load()` を version dispatch 化:`== SCHEMA_VERSION` はそのまま、それ以外は `migrate_to_current` 経由。migration 済み load は `live_revision = saved + 1`(次 save が v5 body を上書きしない implicit step) |
| `ops.rs` | 12 routing variants(`AddBus`/`RemoveBus`/`AddTrackStrip`/`AddConnection`/`RemoveConnection`/`AddSend`/`RemoveSend`/`SetSendTap`/`SetConnectionEnabled`/`InsertProcessor`/`RemoveProcessor`/`ReorderProcessor`)。`RemoveTrack` に strip cascade、`RemoveParameterTarget` に send `gain_target` 参照保護。`AddSend` は Connection+SendConfig を 1 op で生成(1 CAS/1 Undo) |
| `prepare.rs`(gateway) | `op_name` に 12 variant の tag 追加。`op_facts` は fail-closed catch-all が routing ops を拒否 — gateway の routing capability model は下流 issue |
| `md_a.rs` / `midi_source.rs` | schema version 断言を 5 → 6 |
| `tests/routing_sf01.rs` | 15 acceptance テスト(master 厳密 1 件、send 1:1 双方向、stable ID、watermark、serialize/reload、v5 migration 決定性・新 snapshot 名、foreign routing 拒否、Unresolved 保持、empty channel map 拒否、undo、参照削除拒否 + Composite、gain_target 正本、cycle 拒否) |

### 設計上の決定点

- **Well-known master**: `WELL_KNOWN_BASE = 1<<96` 以上の予約 raw 値を
  bootstrap master(bus/strip/chain/in/out)に割当。generator 空間と非交差のため
  既存 ID 断言 test・watermark 契約を壊さず「Master 厳密 1 件」を empty
  project でも成立させた。
- **endpoint-pair dedup の範囲**: `(source,destination)` pair への
  duplicate 拒否は Main/Sidechain のみに適用。Send edge は固有の tap/level
  を持つ並列経路であり、main route と同一 pair を共有するのは合法
  (test が実機会話で発見 — oracle 側の責務で修正)。
- **Migration は projection であって serde default ではない**:
  `#[serde(default)]` は v5 body を *parse* 可能にするだけで、明示的な
  v5→v6 step が legacy "every clip reaches master" を投影する。
- **Channel Map**: persisted されるのは `ExplicitChannelMap` のみ。
  Layout Identity の等価判定は generic engine(#106)の責務で、model は
  `channel_map = None` が同一 layout の既定自動接続を意味し、異 layout は
  明示 map の提示(approval)を要求する契約とした。Layout 型自体は永続化
  しない。

### 受入条件との対応

| AC | 対応 |
| --- | --- |
| Stable Semantic IDs + Track/Strip/Bus/Connection/Send single authority + Ref/Watermark validation | `define_id!` 7 型、`RoutingModel::validate` を `validate_references` に統合、`max_issuable_id_raw` 全 map fold + reserved 除外 |
| 共通 v5 Legacy Project Migration + direct Clip→Master golden compat + 単一 reader + insert_count FX を捏造しない | `migration.rs` ordered registry、`migrate_v5_routing` が track→master `Main` conn を投影、processors は空 |
| Send/Connection 厳密 1:1・孤立/二重参照拒否 | oracle が双方向に検査、`AddSend`/`RemoveSend`/`RemoveConnection` が対を atomic に生成/除去 |
| Unknown/missing plugin・renamed port は明示 `Unresolved` | `ProcessorBinding::Unresolved`/`PortBinding::Unresolved` を persisted、数値一致での付替なし |
| 参照先 Track/Bus 削除は拒否または明示 composite | `RemoveTrack`/`RemoveBus`/`RemoveProcessor` は所有物のみ cascade、dangling ref は post-apply oracle が command 全体を拒否 → `Composite` が正式経路 |
| `SendConfig.gain_target` は `Project::control` の 1 ParameterTarget 正本への参照 | reference のみ + `RemoveParameterTarget` が send 参照を検査 |
| Recording/automation/import/save 非回帰 | workspace 全 test pass(playback/recording/export golden 含む) |
| Ruviform Graph/Mixer/RT 型との分離 | semantic ID は project crate の `define_id!`、runtime ID とは別型 |

## Related Experiments

なし。`docs/impl/exp/` に関連する実験 log file はなく、front
matter `metadata.experiments` の adopted/rejected も空。

## Validation

- 環境: Windows、rust 1.99.0(mise pin)+ MSRV 1.89
- `mise run fmt-check` — Pass
- `mise run check` — Pass(workspace 全 target)
- `mise run lint`(`cargo clippy --workspace --all-targets -- -D warnings`) — Pass
- `mise run test`(`cargo test --workspace --locked`) — Pass、全スイート緑(新規 `routing_sf01` 15 テスト含む)
- `mise run build` — Pass
- `mise run test-msrv`(rust 1.89 `cargo test --workspace --locked`) — Pass

## Risks

- Migration は read-only projection であり、`#[serde(default)]`
  による parse と完全 Migration を混同しない設計。v5 body を
  parse すると `routing` は空になるが migrated ではなく、
  厳密 schema version 検査を保持し v5→v6 は
  `migrate_to_current` 経由のみ。
- `insert_count` 由来の仮想 FX processor は migration で作ら
  ない — 旧 Project の外部 insert FX は processor として
  永続化されず、track 毎の strip+chain+main ports+master への
  `Main` connection のみが投影される（非捏造の意図的契約。旧
  挙動の維持は下流 lowering の責務）。
- routing を既に持つ v5 body は migration が拒否する — 部分的に
  routing が書き込まれた v5 snapshot は v6 へ進めない。
- Gateway `op_facts` は fail-closed catch-all で routing ops を
  拒否する — model は project 層にのみ存在し、routing capability
  model は下流 issue 待ちのため現状 gateway 経由では routing
  操作を発行できない。
- `SendConfig.gain_target` は既存 `ParameterTargetId` への参照
  のみ。`ControlScope`/`ControlOwner` に Strip/Send/Processor
  owner は未導入で、owner mapping は CTL-01 の接続点。

## Follow-ups

- `ControlScope`/`ControlOwner` の `Strip`/`Send`/`Processor` owner 拡張は
  CTL-01/#121 で採用 — 現状 `SendConfig.gain_target` は既存 scope 上の
  target を参照する。
- `AddSend` の ControlTarget 同時生成は CTL-01 採用後に op 拡張として
  接続(現状は既存 target 参照 or `None`)。
- Gateway `op_facts` の routing observable model、runtime lowering
  (SF-03/#106)、plugin capability validation(#106/#101)、
  Temp Ref→Stable ID transaction(SF-02/#105)。
- `doc-driven-dev-graph` の route script が `.apm/skills/` 配下に存在
  しない環境差異 — 文書 chain は canonical `docs/` に手動作成済み。
