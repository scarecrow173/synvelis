---
id: "SPEC-550StripRtOps02"
type: "spec"
status: "approved"
title: "Issue #550 spec — strip routing ops・scope entity removal（CB-06 後続第 2 段）"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/550"
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
    - "docs/discovery/issue-550-strip-routing-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-550-strip-routing-ops.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
---

# Issue #550 spec — strip routing ops・scope entity removal

## 目的

`VariantScope::StripFields` の mask に routing field を追加し、
strip-sourced routing entity の mask-gated candidate edit と
scope entity removal を project adapter・gateway wire・
adopt の 3 経路で一貫して受理する。CB-STRIP-02/03 の
「display truth = enforcement truth」「外部 wiring を
candidate が所有しない」契約を routing 領域へ拡張する。

## 要件

### R1 — mask field の拡張

- `StripField` に `Sends`・`MainRoute`・`Sidechain`・
  `Bus` を追加する（serde `snake_case`:
  `"sends"`/`"main_route"`/`"sidechain"`/`"bus"`）。
- `scope_objects` が field 毎に routing 所有を導出する:
  - `Sends`: source port が scoped strip に属する
    `SendConfig` + 対応 `kind = Send` connection +
    send-owned control closure。
  - `MainRoute`/`Sidechain`: source endpoint が scoped
    strip または scope-owned chain processor に属する
    `Main`/`Sidechain` kind connection。
  - `Bus`: bus-owned strip の場合のみ bus entity。
- foreign-source edge（他 strip からの feed、外部
  sidechain key）は scope 外 shared boundary context の
  ままとする。

### R2 — adapter の op 受理

- mask 宣言済み family に対応する routing op のみ受理:
  `SetSendTap`/`SetSendGainTarget`/`AddSend`/`RemoveSend`/
  `RemoveConnection`（send edge）/`SetConnectionEnabled`/
  `SetConnectionChannelMap`/`AddConnection`・
  `RemoveConnection`（Main/Sidechain edge）/`ConfigureBus`
  （bus-owned strip のみ）。
- source・destination port の双方が scope confinement を
  満たすことを検査し、foreign endpoint は
  `variant-scope-violation` で拒否する。
- mask 未宣言 family の routing op は
  `variant-unsupported` で拒否する。

### R3 — scope entity removal

- adapter の removal check が routing entity
  （`SendConfig`・`RoutingConnection`）と control closure
  （`ParameterTarget`・`AutomationLane`・
  `ModulationSource`・`ModulationAssignment`）を scope
  所有判定で受理する。
- `apply/entity.rs::removal_supported_kind` と
  `remove_entity` が上記 kind を冪等に削除する。
  cascade（send→connection→control closure）は計画済み
  sibling `Removed` write が担い、`remove_entity` は
  scope policy を再導出しない。
- scope root・非対応 kind の削除は従来通り拒否。

### R4 — adopt 経路

- candidate 側削除の `Removed` write は削除対象 entity の
  serialization を payload に載せる。`preflight_ports`
  が `RoutingConnection`/`SendConfig` write の endpoint を
  parse するため、`Null` payload では routing entity の
  adopt が `InvalidPort` で失敗する（発見済み defect の
  修正）。`Added`/`Modified` write の検証は不変。

### R5 — gateway wire 表面

- `synvelis.send.create`/`remove`/`set-tap`/
  `set-gain-target` の 4 capability を op 表に登録し、
  `prepare.rs` で `AddSend`/`RemoveSend`/`SetSendTap`/
  `SetSendGainTarget` へ lower する。`op_facts` は
  read/precondition を正直に宣言する。
- `send.create` の source/destination は
  `semantic-port-*` tagged string args、`gain_target` は
  optional `param-*` id。create parent は `Strip`。
- `ResourceKind::Send` のみが observable な routing
  resource。connection/bus/semantic port は内部構造の
  ままとし、対応 op の wire capability は本 slice では
  提供しない（`UnknownCapability` が正直な応答）。

### R6 — 保存される契約

- live/candidate 分離・branch revision・scope digest・
  provenance・one-CAS adopt は不変。
- gateway lowering は構文的であり、mask/confinement の
  判定は `StripFieldsAdapter` が唯一の権威である。

## 受入条件

- `{Sends}` mask で send tap/gain-target/作成/削除の
  candidate edit が受理され、live は不変。
- `{MainRoute}`/`{Sidechain}`/`{Bus}` で対応 family の
  connection/configure op が受理される。
- foreign-source/foreign-destination の routing op が
  typed scope violation で拒否される。
- mask 未宣言 family が `variant-unsupported` で拒否
  される。
- send 削除の adopt が send・edge・target・automation
  lane を live から除去し、foreign boundary feed を
  byte-identical に保持する。
- gateway で send 4 op の positive wire lowering と
  mask negative・deferred surface negative を実測する。
