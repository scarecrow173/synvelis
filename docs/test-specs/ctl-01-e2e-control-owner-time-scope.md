---
id: "TSPEC-034CTL01Pr2sUv7wY4zAbCdEfG"
type: "test-spec"
status: "approved"
title: "CTL-01 ControlOwner/AutomationTimeScope 検証仕様"
created: "2026-11-05"
updated: "2026-11-05"
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
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CTL-01 検証仕様(`crates/synvelis-project/tests/ctl_01.rs` ほか)

## Feature: 型・座標解決(CTL-01-MODEL)

- Rule: `ProjectMusical` 所有 target (`Project`/`Track`/`Strip`/`Send`/
  `Processor` owner) は project beat をそのまま評価する。
  - Example: `AddParameterTarget{owner: Strip(s), time_scope:
    ProjectMusical}` → `evaluate_control` が pos をそのまま使い
    base/automation を返す。
- Rule: `Clip`/`Note` owner は local scope のみ受理し、
  Clip/Note 以外の owner に `ClipLocal`/`NoteLocal` を指定すると
  write-time で `Error::Validation`。
  - Example: `AddParameterTarget{owner: Track(t), time_scope:
    ClipLocal}` → `Error::Validation`。
- Rule: Clip/Note owner の local 評価座標は旧 `ControlScope` と同じ
  結果(anchor・domain 検査・`ScopeUnavailable`)。
  - Example: musical-anchored clip の `[start,end)` 内/外で新旧
    同一の local pos / エラー。
- Rule: `ProjectAbsolute` は `Error::ScopeUnavailable` を明示返却
  し、beats として評価しない。
  - Example: `evaluate_control` on `time_scope=ProjectAbsolute`
    target → `Error::ScopeUnavailable`。
- Rule: 存在しない Strip/Send/Processor owner は write-time と
  `validate_references` の双方で `Error::NotFound`。
  - Example: `AddParameterTarget{owner: Send(s_unknown)}` →
    `Error::NotFound`、手作り dangling target の load は拒否。

## Feature: role 一意性(CTL-01-ROLE)

- Rule: singleton role (`Fader`/`Pan`/`Mute`/`Solo`/`SendLevel`) は
  同 owner に 1 件のみ。
  - Example: 同 strip に 2 つ目の `role=Fader` target →
    `Error::Validation`。
- Rule: `Generic`/`ProcessorParam` は同 owner に複数共存可。
  - Example: 同 processor に `ProcessorParam`×2 → 受理。
- Rule: `validate_references` も同じ一意性を oracle として検査する
  (手作り project の load 拒否)。

## Feature: Track/Bus→Strip 一意性・gain_target(CTL-01-ROUTING)

- Rule: 1 Track は最大 1 strip(`AddTrackStrip` 二重呼出は拒否)、
  手作り duplicate Track strip は `validate_references` で拒否。
- Rule: Bus↔strip は厳密 1:1(`StripOwner::Bus` 双方向一致) —
  `RoutingModel::validate` 既存 oracle を CTL-01 テストで断言。
- Rule: `SendConfig.gain_target = Some(t)` は `t` が存在し、かつ
  `t.owner == ControlOwner::Send(send.id)` を要求する。
  - Example: `gain_target` が `ControlOwner::Track` target を指す
    手作り project → `Error::Validation`/`NotFound`。
- Rule: routing entity 削除(Bus/Track)で所有 target が dangling
  しない — cascade で除去されるか op が拒否する。
  - Example: `RemoveBus` → bus strip 所有 target は除去済みで
    `validate_references` PASS。

## Feature: Migration・永続(CTL-01-SCHEMA)

- Rule: v6 snapshot は `SCHEMA_VERSION=7` の reader が ladder 経由で
  load し、owner → `ControlOwner`/`time_scope`/`role=Generic` に
  厳密投影される。
  - Example: v6 body の `{"scope":"Clip","id":...}` owner →
    `Clip` + `ClipLocal`;`{"scope":"Track"}` → `Track` +
    `ProjectMusical`。load 後 `evaluate_control` が旧形式で
    serialize した project と同一結果(golden)。
- Rule: v7 save/reload round-trip は owner/role/time_scope を保持し
  `Project` 等値。wire に `time_scope`/`role`/`owner` 形式が出る
  ことを JSON 断言で検証。
- Rule: `id_watermark`・全 live ID・Undo/Redo が変わらない。
  - Example: `AddParameterTarget`→undo で除去、redo で復元、
    save→load 等値。
- Rule: `ControlScope` 文字列・`"scope"` tag は新 serialize に出ない
  (serialize は新形式のみ)。

## Feature: Downstream projection(CTL-01-GW/WS)

- Rule: gateway snapshot JSON は `{"owner": ..., "id": ...}` +
  `"role"`/`"time_scope"` を emit し、`ResourceRef::Strip`/`Send`/
  `Processor` が kind parse/name/enumerate で往復する。
- Rule: `ControlSession::record_points` は新 `resolve_scope_pos`
  arity で project pos → local pos に変換し、domain 外 point を
  捨て、他エラーを伝播する(workspace `pc_c1` 系テストを新 API で
  維持)。
- Rule: `EvalPlan::is_stale` は owner/time_scope の変更を検出する。
