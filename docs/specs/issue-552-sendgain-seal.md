---
id: "SPEC-552SendGainSeal02"
type: "spec"
status: "approved"
title: "Issue #552 spec — SetSendGainTarget rebind の旧 SendLevel seal 解放"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/552"
    - "docs/discovery/issue-552-sendgain-seal.md"
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
    - "docs/discovery/issue-552-sendgain-seal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-552-sendgain-seal.md"
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/specs/issue-550-strip-routing-ops.md"
  refines: []
---

# Issue #552 spec — SetSendGainTarget rebind の旧 SendLevel seal 解放

## 目的

`ProjectOp::SetSendGainTarget` の Some→Some rebind と
Some→None unconfigure で、旧 `SendLevel` seal を同一
apply 内で正しく解放し、`(Send, SendLevel)` singleton
不変条件を保ったまま受理する。sv#550 が typed
`Unsupported` で封じ込めた遷移を positive 経路へ
置き換える。

## 要件

### R1 — seal 解放 semantics（domain）

- `send.gain_target` が `Some(old)` から別値
  （`Some(new)` または `None`）へ遷移するとき、旧
  target が `(owner = Send(send_id), role = SendLevel)`
  の seal を実際に保持していれば、release → seal を
  同一 `Operation::apply` 内で不可分に行う。
- release は旧 target の削除とその排他閉包のカスケード:
  - `old` を `target` とする `AutomationLane` を削除。
  - `old` を `target` または `depth_source` とする
    `ModulationAssignment` を削除。
  - `old` の `ParameterTarget` を削除。
- send-owned modulation source・send が所有する他の
  target（`SendLevel` 以外の role）は削除しない。
- `Some(old) → Some(old)` の再 assert・`None → None`・
  `None → Some` は従来どおり（release は発火しない）。
- apply は scratch clone 上で完了し post-apply oracle
  が最終状態のみを検査するため、release と seal の間に
  観測可能な半端状態は存在しない（1 CAS・1 undo）。

### R2 — adapter の positive 化

- `check_strip_routing_op_targets` の
  `SetSendGainTarget` arm から「previous SendLevel seal
  cannot be released」の typed `Unsupported` を除去し、
  rebind を positive 受理とする。
- 既存の scope 検査は維持: send は scope-owned
  `SendConfig`、新 target は scope-owned
  `ParameterTarget` であること（`require_scope`）。
- release で scope object を離脱する旧 target・lane・
  assignment は `check_strip_fields_scope_removals` が
  supported deletion として受理する — 全 kind が
  `removal_supported_kind` に含まれることを前提とし、
  move-out（entity 残存での scope 離脱）は発生しない
  （entity ごと削除される）。

### R3 — 影響経路

- 低層 `ProjectOp::SetSendGainTarget`・
  `RoutingEdit::SetSendGainTarget`（`routing_tx` 経由）・
  gateway `synvelis.send.set-gain-target`（既 lower 済み）
  は同一 apply を共有するため、domain 修正で全経路が
  自動的に positive になる。wire/gateway 側の変更は
  本 slice では行わない。
- `synvelis#122` の 1:1 契約を保持: rebind 後も
  `gain_target` は send 所有の `ParameterTargetId` を
  1:1 参照し、send owner の `SendLevel` target は高々
  1 件。旧 target の削除により孤立した Persisted Gain
  は残らない。

### R4 — 保存される契約

- persisted 形状・schema version・snapshot 形式は不変。
- undo/redo は従来どおり apply 全体を 1 エントリとして
  復元する（release + seal + 参照付け替えが不可分）。
- `RemoveSend`・`AddSend`・他 routing op の semantics は
  不変。

## 受入条件

- `SetSendGainTarget` の Some→Some rebind が旧
  `SendLevel` seal を解放して受理される。
- Some→None unconfigure が孤立 `SendLevel` を残さず、
  後続の None→Some（別 target）が受理される。
- rebind 中の半端な seal 状態（旧 seal 残存・新 seal
  未確定）が発生しないことを headless で実測する。
- 低層 op 経路と `RoutingEditIntent` 経路の両方で
  positive を pin する。
- strip-fields candidate edit で rebind が受理され、
  旧 target の離脱が supported removal として扱われる
  （typed `Unsupported` が positive 経路に置き換わる）。
- `mise run check/lint/test synvelis-project`・
  `mise run fmt-check`・`prek run --all-files` が
  exit 0。
