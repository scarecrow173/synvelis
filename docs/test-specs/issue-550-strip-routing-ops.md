---
id: "TSPEC-550StripRtOps07"
type: "test-spec"
status: "approved"
title: "Issue #550 test-spec — strip routing ops・entity removal の検証仕様"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/550"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-550-strip-routing-ops.md"
    - "docs/designs/issue-550-strip-routing-ops.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-550-strip-routing-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/tasks/issue-550-strip-routing-ops.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-550-strip-routing-ops.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #550 test-spec — strip routing ops・entity removal の検証仕様

## 検証対象

`cb06_strip_fields.rs` の #550 節（project 軸）と
`cb06_strip_gateway.rs` の send wire 試験（gateway 軸）。

## Rules

- R1: `{Sends}` mask で `SetSendTap`/`SetSendGainTarget`/
  `AddSend`/`RemoveSend`/send edge の
  `SetConnectionEnabled`/`SetConnectionChannelMap`/
  `RemoveConnection` が candidate edit として受理され、
  send-owned `SendLevel` target の作成・bind・base・
  automation 閉包も受理される
- R2: `{MainRoute}`/`{Sidechain}`/`{Bus}` で対応 kind の
  connection/configure op が受理される（bus は
  bus-owned strip のみ）
- R3: foreign-source send/connection・foreign bus
  configure・mask 未宣言 family は typed reject
  （`variant-scope-violation`/`variant-unsupported`）
- R4: `boundary_dependencies` が mask-relative —
  `{Fader}` では main/send edge が boundary、
  `{Sends}`/`{Sends,MainRoute,Inserts}` では宣言 family
  の edge が scope 所有となり foreign feed のみ残る
- R5: send 削除の adopt が send・edge・send-owned
  target・automation lane を live から除去し、
  candidate-local target は live に漏れず、foreign
  sidechain feed は byte-identical に保持される。
  adopt 前の live は無変更
- R6: gateway で `synvelis.send.{create,remove,set-tap,
  set-gain-target}` が positive に lower・commit され、
  live send truth（tap・gain binding）は不変
- R7: 狭 mask（`sends` 未宣言）での send op は
  `variant-unsupported`、`synvelis.connection.*`/
  `synvelis.bus.*` は capability 非存在で
  `UnknownCapability`（deferred surface の正直な応答）
- R8: `capabilities()` 報告が mask-relative — 宣言
  family は Supported、未宣言 routing family・`Bus`
  （track strip）・`preset_select` は Unsupported、
  `entity_remove` は非空 mask で Supported

## Examples

- `cb_strip_550_sends_ops_mask_gated` — `{Sends}` で
  tap/connection/target bind/第 2 send 作成・削除
- `cb_strip_550_route_and_bus_ops_per_field` —
  MainRoute・Inserts+Sidechain・Bus を別 fixture で
- `cb_strip_550_routing_confinement_and_mask_negatives` —
  `{Fader}` 未宣言・foreign-source・foreign bus 拒否
- `cb_strip_550_send_removal_closure_and_adopt` —
  削除カスケード adopt + foreign feed 保持
- `cb_strip_550_boundary_dependencies_mask_relative` —
  boundary の mask-relative 証拠
- `cb06_strip_fields_scope_boundary_and_capabilities` —
  capability/boundary 報告の mask-relative 更新
- `cb06_strip_550_send_ops_wire_lowering` — 4 op の
  positive wire lowering
- `cb06_strip_550_send_ops_mask_and_surface_negatives` —
  mask negative + deferred surface negative

## Oracle 種別

headless（`mise run test synvelis-project`・
`mise run test synvelis-gateway`）。実 PCM/RT/plugin・
実 GUI lane は `NotRun`（外部 gate 責務）。
