---
id: "TASK-034SF02Yb2tQr9sV2xD7eFhIj"
type: "task"
status: "done"
title: "SF-02 T1 — RoutingTransaction/TempRef staging・channels・mute/solo authority・汎用検証投影"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/105"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-02-routing-transaction.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/designs/sf-02-routing-transaction-design.md"
    - "docs/adr/sf-02-routing-transaction-port-channels-mute-solo-authority.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-02-e2e-routing-transaction.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-02 T1 — RoutingTransaction staging と model 拡張

## T1.1 Model additive 拡張

- `SemanticPort.channels: u32`(`#[serde(default)]`, default 2)。
- `ParameterRole::Mute`/`Solo` 追加・`ControlOwner::Strip` 向け
  mute/solo target 解決 helper。
- `RoutingModel::strip_muted/strip_soloed/audible_strips`(enabled
  接続のグラフ投影上で mute/solo 評価)。
- `SendConfig` 拡張なし(gain は target のみ)。

## T1.2 routing_tx.rs

- TempBusId/TempSendId/TempConnectionId、BusRef/StripRef/SendRef/
  ConnectionRef/PortRef、`RoutingEditIntent`/`RoutingEdit`/
  `SendGainSpec`/`ExplicitConversionPolicy`、`RoutingEditError`、
  `RoutingPlan`/`RoutingReceipt`。
- `preflight(base, intent, ids)`: TempRef 事前割当→既存 apply helper で
  候補 Project を順次構築→全 invariant + 意味検証 + 汎用検証。
- `ProjectOp::Routing { intent }` を validate/apply へ統合。
- `SetSendGainTarget`/`ChangeBusConfiguration`/`SetMute`/`SetSolo` を
  RoutingEdit と ProjectOp 両面で提供。

## T1.3 意味検証 + 汎用検証

- `to_graph_spec(&RoutingModel) -> ruviform_graph::GraphSpec` 投影 +
  `Validator` 呼出(direction/domain/layout/multiplicity/cycle)。
- DAW 規則: ownership・role/kind・Send 1:1・duplicate pair・map consent・
  unresolved port・全 cycle 拒否・generation 一致。
- `ruviform-graph` を `synvelis-project` 依存に追加。

## T1.4 gateway 統合

- `op_name`/`op_facts` に `"routing"` arm と ResourceRef 記録を追加。

## 完了条件

- `mise run check` で workspace 型チェック通過(後続テストは T2)。
