---
id: "DESIGN-550StripRtOps03"
type: "design"
status: "approved"
title: "Issue #550 design — strip routing mask・confinement・wire 表面"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/550"
    - "docs/specs/issue-550-strip-routing-ops.md"
    - "docs/discovery/issue-550-strip-routing-ops.md"
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
    - "docs/specs/issue-550-strip-routing-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-550-strip-routing-ops.md"
    - "crates/synvelis-project/src/variant/scope.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
---

# Issue #550 design — strip routing mask・confinement・wire 表面

## 全体構造

3 層の責務分離は CB-06/548 のまま維持する:

- **`variant/scope.rs`（所有 oracle）**: `scope_objects` が
  `StripField` 毎の routing 所有を導出。fingerprint・
  adopt・confinement が同じ集合を共有する。
- **`variant/adapter.rs` + `strip.rs`（判定権威）**:
  mask 宣言・op 受理・scope confinement・capability 報告・
  boundary dependencies。
- **`synvelis-gateway`（構文的 lowering）**: op 表・
  `lower_for`・`op_facts`。mask を再判定しない。

## scope 所有の導出規則

routing connection の帰属は **source endpoint の所有**で
決定的に一意化する:

- `Sends`: `SendConfig` の source port owner が scoped
  strip のもの。対応 `kind = Send(id)` connection と
  send-owned control closure（`SendLevel` target・その
  automation/modulation）を同時に所有する。
- `MainRoute`/`Sidechain`: connection.kind が
  `Main`/`Sidechain` で、source port owner が scoped
  strip または `Inserts` 所有の chain processor のもの。
- `Bus`: `StripOwner::Bus(bus_id)` の strip に対し bus
  entity を所有（track-owned strip では空集合）。
- destination 側 endpoint が scope 外でも source 所有で
  帰属は決まる — ただし adapter の confinement 検査で
  foreign destination への新規接続は拒否する。

`boundary_dependencies` は「scope-affiliated port を
持つが scope object でない connection」を返すため、
mask 宣言で boundary 集合が縮む（mask-relative）。

## adapter の検査順序

1. `require_strip_field` — mask が family を宣言するか
   （未宣言 → `variant-unsupported`）。
2. `strip_routing_source` — op の source 側 entity が
   scoped strip（または scope-owned processor/bus）に
   帰属するか。
3. endpoint confinement — destination port が scope 外
   entity（foreign strip/bus の入力）を指す場合の
   新規接続・付け替えを `variant-scope-violation` で
   拒否。既存 scope-owned edge の enable/channel-map
   編集は destination が boundary でも受理する
   （辺の再配線ではなく辺の属性編集）。
4. `PreEditIds` に作成予定の send/connection を登録し、
   post-edit でも同じ confinement を検査する。

## removal の責務分界

- `remove_entity`（apply/entity.rs）は kind の存在を
  知っている限り冪等に削除するだけ — scope policy を
  再導出しない。
- cascade は `plan_adopt_writes`/`plan_branch_writes`
  が計画する sibling `Removed` write が担う
  （send→connection→`Send`-owned control closure）。
- `removal_supported_kind` に
  `ParameterTarget`/`AutomationLane`/`ModulationSource`/
  `ModulationAssignment`/`RoutingConnection`/`SendConfig`
  を追加。scope root・clip/track 等の構造 entity は
  従来通り非対応。

## adopt の Removed write payload

`preflight_ports` は `RoutingConnection`/`SendConfig` の
write payload から endpoint を parse し、port 実在と
direction を検証する。`ChangeKind::Removed` の payload
は `remove_entity` が読まないため、`plan_adopt_writes`
（lease 内 `variant/engine.rs`）が削除対象 entity の
live serialization（`scope::scope_value`）を載せる。
これで preflight は削除対象の endpoint を正しく解決
でき、`Added`/`Modified` の検証は一切緩めない。

## gateway の表面選定

`ResourceKind` に `Connection`/`Bus`/`SemanticPort`
が無く、追加は `snapshot::ENUMERATED_KINDS` 等の非
lease 面の拡張を要する。よって本 slice の wire 表面は
`Send` のみ:

| capability | target | args | lower |
| --- | --- | --- | --- |
| `synvelis.send.create` | create(parent=Strip) | source/destination port id 文字列, tap enum, gain_target? | `AddSend` |
| `synvelis.send.remove` | resource(Send) | — | `RemoveSend` |
| `synvelis.send.set-tap` | resource(Send) | tap enum | `SetSendTap` |
| `synvelis.send.set-gain-target` | resource(Send) | gain_target（null 可・必須 key） | `SetSendGainTarget` |

port id は resource model が名付けられないため
`semantic-port-*` tagged string を `str_` schema +
`SemanticPortId::from_tagged` で検証する
（`insert-processor` の ports posture と同型）。
connection/bus op は capability 非登録のまま —
`UnknownCapability` が deferred surface の正直な応答。

## テスト方針

- project 軸（`cb06_strip_fields.rs`）: mask 別の
  positive/negative、confinement、send control closure、
  adopt の削除カスケードと foreign feed 保持、
  boundary の mask-relative 性。
- gateway 軸（`cb06_strip_gateway.rs`）: send fixture
  （send + bound `SendLevel` target を controller op で
  構築）での 4 op positive lowering、狭 mask negative、
  deferred surface negative、capability/boundary 報告の
  mask-relative 更新。
