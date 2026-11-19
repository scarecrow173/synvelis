---
id: "DISC-552SendGainSeal01"
type: "discovery"
status: "resolved"
title: "Issue #552 discovery — SetSendGainTarget rebind の旧 SendLevel seal 解放"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/552"
    - "https://github.com/scarecrow173/synvelis/issues/550"
    - "https://github.com/scarecrow173/synvelis/issues/122"
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
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/specs/issue-550-strip-routing-ops.md"
---

# Issue #552 discovery — SetSendGainTarget rebind の旧 SendLevel seal 解放

## 背景

`synvelis#550`（PR `synvelis#551`）で strip-fields scope の
routing op 面が着地した際、`SetSendGainTarget` の
Some→Some rebind が pre-existing 不具合として実証された。
send の gain target は `ParameterRole::SendLevel` の
`(owner, role)` singleton であり、apply は新 target に
`owner = Send(send_id)`・`role = SendLevel` を seal するが
旧 target の seal を解放しない。結果として旧・新 2 target
が同一 send owner の `SendLevel` を共有し、post-apply
oracle（`validate_control_binding` の singleton 検査）が
必ず reject する。sv#550 では
`variant/adapter.rs` の `check_strip_routing_op_targets`
で typed `Unsupported` として fail-closed 封じ込めた。

`synvelis#552` はこの封じ込めの根本原因 — 旧 seal の正しい
解放 semantics — を扱う。

## 調査結果

### 1. seal の構造と oracle の拒否点

- `SendConfig.gain_target` は send が所有する既存
  `ParameterTargetId` への 1:1 参照（`synvelis#122` 契約 —
  第 2 の Persisted Gain を持たない）。
- `ParameterRole::SendLevel` は singleton role
  （`control.rs` `is_singleton`）で、`validate_control_binding`
  は同一 `(owner, role)` の重複を `Error::Validation`
  で拒否する。post-apply oracle は全 target にこれを
  適用するため、旧 seal 残存の rebind は commit できない。
- 既存 test は `gain_target: None` の未構成 send
  （`adopt_fixture`）しか使わず、None→Some のみ検証して
  いた。Some→Some・Some→None→Some は未踏。

### 2. scope 側の制約が解放 semantics を絞る

strip-fields scope の所有導出（`variant/scope.rs`
`strip_routing_objects`）は、scope-owned send が所有する
**全** control entity（role 不問の `ControlOwner::Send`
target・send-owned modulation source・それらの
lane/assignment 閉包）を scope object とする。

candidate edit は apply 後に `before_objects −
after_objects` を `removed` として検査し、各 object が
`removal_supported_kind` かつ candidate から実削除済み
であることを要求する（`check_strip_fields_scope_removals`）。
したがって:

- **削除経路**: 旧 target + その閉包が `removed` に
  現れても `ParameterTarget`/`AutomationLane`/
  `ModulationAssignment` は全て removal-supported であり、
  adopt の `Removed` write 経路（sv#550）もそのまま表現
  できる。
- **unseal 経路**: owner を send 以外へ動かすと entity は
  scope object 集合から落ちるが実体は残るため
  「move-out」として typed reject される。owner=Send の
  まま role のみ Generic へ落とす変形は scope に残るが、
  「send が所有する level でない param」という domain が
  定義していない中間状態を新設し、付随する automation
  を誰も読まない形で残す。

### 3. 先例との整合

- `RemoveSend` は `remove_control_owned` で send 所有の
  control 閉包（target・source・lane・assignment）を
  まとめて削除する — seal が解除されるときその閉包も削除される、
  という既存 semantics。
- `RemoveConnection` の `Send` kind も `SendConfig` を
  道連れに削除する（1:1 相互参照）。
- sv#550 の adopt 実測でも「send 削除が send・edge・
  target・automation lane を live から除去する」ことが
  受理済み。

## 決定

`SetSendGainTarget` apply で、`send.gain_target` が
`Some(old)` から別値（`Some(new)` または `None`）へ
遷移するとき、旧 target が実際に
`(owner = Send(send_id), role = SendLevel)` の seal を
保持していれば、その target と排他閉包（その target を
指す automation lane・`target`/`depth_source` として
参照する modulation assignment）を同一 apply 内で削除
する。新 target への seal は従来どおり同じ apply で
行い、release → seal の順で適用する。これにより:

- Some→Some rebind・Some→None unconfigure・
  Some→None→Some の全遷移で `(Send, SendLevel)`
  singleton が apply 後に高々 1 件となる。
- 半端な seal 状態（旧 seal 残存・新 seal 未確定）は
  apply が scratch clone 上で不可分に完了し
  post-apply oracle が最終状態だけを検査する構造上
  commit され得ない。
- send-owned modulation source は削除しない —
  `remove_control_owned` が owner 削除時に全 source を
  一括削除するのに対し、本 slice は単一 target の解放であり、
  source は他 target を drive し得る独立 entity で
  未 assign の source は合法状態。

send が `SendLevel` 以外の target/source を追加所有
し得るため（`AddParameterTarget`/`AddModulationSource`
は `owner = Send` を受理する）、解放は owner 単位の
`remove_control_owned` ではなく旧 gain target 単位の
カスケードとする。

## 非目標

- send 以外の seal/rebind 系 op の横断見直し（Issue 対象外）。
- 旧 gain の automation を新 target へ移送する semantics
  （付け替え時の補間は別 Issue の領域 — 本 slice は
  閉包ごと削除で確定）。
- gateway wire surface の変更（`synvelis.send.set-gain-target`
  は既に lower 済みで domain 修正により自動的に通る）。
- cb-06 系文書・`synvelis-gateway` 側の占有面の更新
  （`synvelis#553` が同時進行）。
