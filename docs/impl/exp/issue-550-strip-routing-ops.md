---
id: "EXP-550StripRtOps09"
type: "experiment"
status: "completed"
title: "Issue #550 impl EXP — strip routing ops 実験記録"
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
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/cb-06-track-bus-adapter.md"
    - "docs/impl/exp/issue-548-strip-domain-lowering.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #550 impl EXP — strip routing ops 実験記録

## E1: wire 表面の選定 — Send のみ

- **試行**: `ResourceKind::Connection`/`Bus`/
  `SemanticPort` を追加して connection/bus op も wire 化
  する案を検討。
- **結果**: `ResourceRef`/`ResourceKind` の exhaustive
  consumer（`snapshot::ENUMERATED_KINDS`・detail/resource
  map 等）が非 lease file に広く、kind 追加は snapshot
  enumeration の意味論変更を伴う。Send は既に observable
  で `ResourceRef::Send` の detail/resource map 対応も
  済んでいた。
- **決定**: wire 表面は `synvelis.send.*` 4 op に限定。
  connection/bus op は adapter 受理のみで wire は
  deferred（`UnknownCapability` が正直な応答）。port id
  は `semantic-port-*` tagged string args で運ぶ
  （`insert-processor` の ports posture と同型）。

## E2: Removed write payload — adopt preflight defect

- **観測**: `cb_strip_550_send_removal_closure_and_adopt`
  が `InvalidPort { detail: "connection … has no
  source" }` で失敗。
- **原因追跡**: `plan_adopt_writes` は削除を
  `ChangeKind::Removed` + `payload: Null` で emit。
  `apply/plan.rs::preflight_ports`（非 lease）は
  `RoutingConnection`/`SendConfig` の全 write payload
  から endpoint を parse するため、`Null` payload の
  Removed write が preflight で落ちる。`remove_entity`
  は payload を読まない — validation と apply の
  payload 前提がずれていた。
- **修正**: lease 内の `variant/engine.rs` で Removed
  write の payload に `scope::scope_value(live,
  origin)`（削除対象 entity の serialization）を載せる。
  `Added`/`Modified` の検証は不変、`remove_entity` の
  挙動も不変。payload を読む consumer は preflight
  のみであることを確認済み。

## E3: routing 所有の帰属規則 — source-side

- **検討**: connection の scope 帰属を source/destination
  どちらで決めるか。destination 所有にすると strip→master
  main route が常に boundary となり `{MainRoute}` が
  実質空になる。source 所有なら strip が自分の送出 edge
  を所有でき、destination（foreign input）は boundary
  context に残る。
- **決定**: source endpoint 所有で帰属を一意化。
  confinement は adapter が別途検査し、foreign
  destination への新規接続・付け替えを拒否するため
  「所有」と「配線許可」を分離できる。

## E4: gateway test fixture — send の構築

- **課題**: `AddSend` は `SendId` を apply 時に mint
  するため、fixture 構築中に send id を事前参照できず、
  `send.set-tap`/`remove` の wire target を決められない。
- **解決**: `strip_send_controller()` で
  `AddSend(gain_target: None)` → live から send id を
  discovery → `AddParameterTarget(owner=Send(send),
  role=SendLevel)` → `SetSendGainTarget` の 3 op で
  bound send を構築（head rev 5）。wire test は
  `routing.sends`/`control.targets`/`routing.ports`
  から project truth を引き、id を hard-code しない。
  `send.create` は create した send の id が wire から
  参照不能なため terminal op として positive lowering
  のみ実測（semantic は project test が担保）。

## E5: capability 報告の mask-relative 化

- `entity_remove` は synvelis#550 で全非空 mask
  Supported（scope-owned entity の削除は常に表現可能）。
  `sends`/`main_route`/`sidechain`/`bus` は field_cap
  で mask-relative、`routing` aggregate はいずれかの
  routing field 宣言で Supported。`bus` は
  `field_cap` のまま — capabilities() は mask のみを
  見るため track-owned strip での空所有は scope 側の
  問題として残る（宣言しても所有 object が無いだけ）。

## E6: review で発覚した confinement/observability の修正

- **gain_target の scope 検査漏れ**: apply の
  re-own 意味論を読み、adapter 層で target の scope
  所有を要求する形に修正。foreign target の採用が
  live 所有権を奪取する経路を遮断した。
- **rebind 不可能 bug の封じ込め**: `SendLevel`
  singleton + seal 非対称により Some→Some rebind は
  apply 段で必ず失敗すると判明（`ops.rs`、本 diff
  以前から存在）。adapter で typed `Unsupported` に
  して wire/candidate 双方で分かりやすい拒否に。
  根本修正は domain 設計を要するため別 Issue 候補。
- **op_facts の cascade 検査**: `remove_control_owned`
  の削除対象を列挙し、resource model で観測不能な
  `ModulationSource`/`Assignment` の存在時のみ
  `unsupported` を立てる条件付き fail-closed —
  通常の send.remove は従来通り prepare 可能。
- **variant.rs の inner facts drop**: inner op の
  `unsupported`/`preconditions` が捨てられる既存 gap
  を発見。非 lease のため本 diff では触れず、別 Issue
  候補として記録（影響は send 限らず全 cascade op）。
