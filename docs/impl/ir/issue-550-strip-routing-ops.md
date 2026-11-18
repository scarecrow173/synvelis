---
id: "IR-550StripRtOps08"
type: "impl"
status: "completed"
title: "Issue #550 impl IR — strip routing ops・entity removal 実装記録"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/550"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-550-strip-routing-ops.md"
      - type: "file"
        path: "docs/specs/issue-550-strip-routing-ops.md"
      - type: "file"
        path: "docs/designs/issue-550-strip-routing-ops.md"
      - type: "file"
        path: "docs/plans/issue-550-strip-routing-ops.md"
      - type: "file"
        path: "docs/tasks/issue-550-strip-routing-ops.md"
      - type: "file"
        path: "docs/test-specs/issue-550-strip-routing-ops.md"
      - type: "file"
        path: "docs/impl/exp/issue-550-strip-routing-ops.md"
      - type: "dir"
        path: "evidence/sv550/"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/scope.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/strip.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb06_strip_fields.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/rig/mod.rs"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "evidence/sv283/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv283/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-550-strip-routing-ops.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-550-strip-routing-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-550-strip-routing-ops.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #550 impl IR — strip routing ops・entity removal 実装記録

## 実装サマリ

`VariantScope::StripFields` の mask に routing field
（`Sends`/`MainRoute`/`Sidechain`/`Bus`）を追加し、
strip-sourced routing entity の所有導出・adapter 受理・
scope entity removal・gateway send wire lowering を
実装した。CB-06/548 の責務分離（scope.rs = 所有 oracle、
adapter = 判定権威、gateway = 構文的 lowering）は維持。

## project 側

- `variant/types.rs`: `StripField` に 4 variant 追加
  （doc comment に synvelis#550 の所有規則を明記）。
- `variant/scope.rs`: `scope_objects` が field 毎に
  routing 所有を導出 — `Sends` は scoped strip source
  の `SendConfig`+`Send` connection+send control closure、
  `MainRoute`/`Sidechain` は source endpoint 所有の
  `Main`/`Sidechain` connection、`Bus` は bus-owned
  strip の bus entity。
- `variant/adapter.rs`: routing op の mask gate・
  source/destination port confinement
  （`strip_routing_source`・`require_strip_field`）・
  scope-entity removal check を追加。doc の stale 記述
  （routing ops unsupported）を更新。
- `variant/strip.rs`: `StripFieldsCapabilities` に
  `sends`/`main_route`/`sidechain`/`bus` per-field cap・
  `routing` aggregate・`entity_remove` Supported を追加。
  `boundary_dependencies` は mask-relative。`edit_candidate`
  が routing op を dispatch し、`PreEditIds` が作成予定の
  send/connection を post-edit confinement に登録。
- `variant/engine.rs`: `plan_adopt_writes` の `Removed`
  write が `scope::scope_value(live, origin)` の entity
  serialization を payload に載せる（`preflight_ports`
  の endpoint parse 対応。`remove_entity` は payload
  不読）。
- `apply/entity.rs`: `removal_supported_kind` に
  `ParameterTarget`/`AutomationLane`/`ModulationSource`/
  `ModulationAssignment`/`RoutingConnection`/`SendConfig`
  を追加し、`remove_entity` に対応 arm を実装（冪等・
  cascade は sibling `Removed` write が担う）。

## gateway 側

- `prepare_engine.rs`: `synvelis.send.create`（create
  parent=Strip, creates=Send）/`remove`/`set-tap`/
  `set-gain-target` の 4 descriptor を op 表に登録。
  comment で resource model の制約（Send のみ observable、
  connection/bus/port は内部構造）と mask 権威の所在を
  明記。
- `prepare.rs`: `target_send`/`arg_semantic_port`/
  `arg_gain_target` helper と
  `lower_send_*` 4 fn・`lower_for`・`op_facts` arms
  （send-exists read・send-ports-exist・
  send-gain-target-exists/adoptable precondition）。

## test 側

- `cb06_strip_fields.rs`: #550 節 6 test —
  `{Sends}` positive（tap/connection/target bind/第 2
  send 作成・削除）、per-field route/bus positive、
  confinement+mask negatives、send 削除 adopt
  （cascade・live 不変・foreign feed 保持）、boundary
  mask-relative。既存 2 件の stale assertion 修正
  （candidate-local lane・`RemoveAutomationLane` が
  scope-owned lane で許可）。
- `cb06_strip_gateway.rs`: send fixture
  （`strip_send_controller` — AddSend + Send-owned
  SendLevel target + bind、head rev 5）・4 op builder・
  `cb06_strip_550_send_ops_wire_lowering`・
  `cb06_strip_550_send_ops_mask_and_surface_negatives`。
  `strip_scope()`/`strip_domain_scope()` を widest
  honest mask（8 field）へ拡張、boundary/capability
  test を mask-relative に更新、lifecycle の scope
  assertion を `strip_domain_scope()` 参照へ。
- `tests/rig/mod.rs`: `tagged()` を全 `ResourceRef`
  variant へ拡張（`create_target` の Strip parent 等）。

## docs/evidence

- doc chain 8 type（discovery/spec/design/plan/tasks/
  test-specs/impl ir/impl exp）を `issue-550-strip-
  routing-ops` slug で作成。generated index は未編集
  （Coordinator が統合時に再生成）。
- cb-06 doc 3 件（impl ir/impl exp/test-specs）に
  synvelis#550 の追補を追加。
- `evidence/sv550/verification.md` 新設、
  `evidence/sv283/` の ledger/verification に追記。

## review 指摘対応（synvelis#550 review）

- **gain_target confinement（BLOCKER）**: `AddSend`/
  `SetSendGainTarget` の `gain_target` が scope 未検査
  だった。apply は適格 target を send へ re-own する
  ため、foreign Send-owned `SendLevel` target の採用が
  adopt 時に live の所有権を奪取し得た。
  `check_strip_routing_op_targets` で
  `require_scope(ParameterTarget)` を追加し、
  scope-owned（send control closure）のみ受理。
  negative test: key-strip send の unbound
  `SendLevel` target を `SetSendGainTarget`/`AddSend`
  へ bind → `ScopeViolation`。
- **send gain rebind（BLOCKER→containment）**:
  `SetSendGainTarget` の Some→Some（別 target）は、
  apply が旧 target の `SendLevel` seal を解放しない
  ため常に `SendLevel` singleton 違反で失敗する
  pre-existing bug（`ops.rs`、非 lease）。adapter で
  「現在 bind と異なる target への再 bind」を
  `Unsupported` で fail-closed。正規の修正（解放先の
  role/owner の domain 設計）は別 Issue 候補。
  `None`→`Some`・`Some`→`None`・同一 target 再
  assert は従来通り受理。
- **send.remove cascade under-report（BLOCKER）**:
  `RemoveSend` は send-owned target/lane（列挙可能で
  diff に Delete が出る）に加え、send-owned
  `ModulationSource`/`ModulationAssignment`（resource
  model 外）を cascade 削除する。`op_facts` の
  `RemoveSend` arm で pinned project を検査し、
  観測不能 entity が存在すれば `unsupported` を立て
  fail-closed（SEC-14）。`SetSendTap` は分離。top-level
  prepare 経路で deny を実測
  （`cb06_strip_550_send_remove_unobservable_
  cascade_denied`）。**残存 gap（非 lease）**:
  `variant.rs` の `intent_facts` が inner op の
  `unsupported`/`preconditions` を捨てるため、
  variant-edit 経路では依然伝播しない — 全 inner op
  共通の pre-existing 系統的 gap であり別 Issue 候補。
- **send.create parent 不整合（NOTE）**: contract
  parent を `Strip`→`Project` へ修正。Send の
  snapshot attrs owner は `Project` であり、
  `CreateEffect.parent` と `create_within` の両軸が
  一致する（`track.create` と同じ posture）。
  lowering の `parent_strip` は撤去。
- **capabilities() doc（NOTE）**: mask-relative の
  誠実な記述へ修正（`Supported` は宣言 contract 内を
  意味し、編集 object の存在は保証しない）。
