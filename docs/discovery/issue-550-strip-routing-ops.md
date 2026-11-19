---
id: "DISC-550StripRtOps01"
type: "discovery"
status: "resolved"
title: "Issue #550 discovery — strip routing ops・scope entity removal（CB-06 後続第 2 段）"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/550"
    - "https://github.com/scarecrow173/synvelis/issues/548"
    - "https://github.com/scarecrow173/synvelis/issues/283"
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
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
---

# Issue #550 discovery — strip routing ops・scope entity removal

## 背景

CB-06（`synvelis#283`）の `VariantScope::StripFields` は
channel 系 field（`Fader`/`Pan`/`Mute`/`Solo`/`Inserts`）の
mask-gated candidate edit を実装した。`synvelis#548`
（PR `synvelis#549`）で strip-domain inner op の wire
lowering を追加し、channel/inserts 系の positive mask-gated
edit が gateway commit-gate 経路で実測された。

`StripFieldsAdapter` の doc・capability 報告は当初から
routing op と scope entity removal を `Unsupported` と宣言
しており、両者は意図的な gap として残されていた。
`synvelis#550` はその 2 系統を埋める後続 slice である。

## 調査結果

### 1. routing op の受理面は project 側に既に半分ある

`ProjectOp` には send/connection/bus 系の op が揃っている:
`AddSend`/`RemoveSend`/`SetSendTap`/`SetSendGainTarget`/
`AddConnection`/`RemoveConnection`/`SetConnectionEnabled`/
`SetConnectionChannelMap`/`ConfigureBus`。
`AddSend` は send config と `kind = Send` connection を
atomic に生成し、gain target の owner を `Send(send_id)` に
seal する（issue-#122 契約）。

`scope_objects`（`variant/scope.rs`）は scope 所有 oracle
であり、routing entity の帰属をここに追加すれば adapter・
fingerprint・adopt が同じ真実を共有できる。

### 2. wire resource model の制約が表面を決める

`ResourceKind`/`ResourceRef` は `Send` を observable
resource として持つが、`Connection`/`Bus`/`SemanticPort`
の kind は存在しない。`snapshot::ENUMERATED_KINDS` 等の
非 lease 面を拡張しない限り、connection/bus op の wire
capability は宣言できない。port id は `semantic-port-*`
tagged string として args に載せる形が既存の
`insert-processor`（ports は内部構造）と整合する。

### 3. adopt の Removed write は payload を載せられない

`plan_adopt_writes` は candidate 側削除を
`ChangeKind::Removed` + `payload: Null` で emit する。
`apply/plan.rs::preflight_ports`（非 lease）は
`RoutingConnection`/`SendConfig` の全 write payload から
endpoint を parse するため、routing entity の Removed
write が `InvalidPort` で preflight 拒否される。
`remove_entity` は payload を読まないので、Removed write
に削除対象 entity の serialization を載せるのが最小の
整合修正（`variant/engine.rs`、lease 内）。

### 4. 境界 semantics は source-side 所有

routing connection の scope 帰属は **source endpoint の
所有**で決める: strip-sourced send/main/sidechain edge
は scope 所有、destination 側（foreign strip の入力、
master への feed を受ける側）は shared boundary context
のまま。これにより candidate は自分の send/main/sidechain
系のみ編集でき、外部 wiring・shared routing context を
所有しない。

## 決定

- `StripField` に `Sends`/`MainRoute`/`Sidechain`/`Bus`
  を追加し、`scope_objects` が field 毎の routing 所有を
  導出する。
- adapter は mask 宣言済み family の routing op のみ受理
  し、source/destination port の scope confinement を
  typed violation で拒否する。
- scope entity removal を adapter + adopt 経路で有効化
  し、`apply/entity.rs` に残りの kind の削除 arm を追加。
- gateway wire 表面は `ResourceKind::Send` を持つ
  `synvelis.send.{create,remove,set-tap,set-gain-target}`
  の 4 capability に限定。connection/bus op は
  `ResourceKind` 非存在のため adapter 受理のみで wire
  表面は deferred とする。

## 非目標

- `ResourceKind::Connection`/`Bus` の追加と
  `snapshot::ENUMERATED_KINDS` の拡張（非 lease 面・
  別 slice）。
- bus-owned strip の `ConfigureBus` wire 化。
- routing の audio semantics 変更（routing legality・
  fan-in/fan-out 規則は既存 engine のまま）。
