---
id: "EXP-sv511iJ1kL3mN5oQ7rS9tU1v"
type: "experiment"
status: "completed"
title: "issue-511-cb07-materialization — 実験記録"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/impl/ir/issue-511-cb07-materialization.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-511-cb07-materialization.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/issue-511-cb07-materialization-exp.jsonl"
  defers: []
  deferred-by: []
---

# issue-511-cb07-materialization — 実験記録

## E1 Removed write の cascade 範囲

**仮説**: Clip 削除時の note cascade は scope-owned note の
`Removed` write に依存させず、delete arm 内で `ProjectOp::
RemoveClip` と同一 semantics を mirror すべきか。

- 調査: clips-only mask（`fields.notes = false`）では note は
  scope object ではなく `Removed` write が計画されない。
  candidate 側では `RemoveClip` op が note を cascade 削除済み
  のため、live が note を残すと `validate_references` で
  dangling `note.clip` となり adopt が失敗する。
- 案 A: delete arm で cascade（採用）— live/candidate が同一
  参照整合に収束。scope 拡大ではなく宣言削除の必須 cascade。
- 案 B: cascade を reconcile_membership に分離 — `Removed` write
  の after entity が存在せず ordering が複雑化。却下。

**観察**: `cb_arr_16_sweep_delete_adopts_and_cascades_notes` で
clips-only mask の sweep 削除が `objects == 1`（clip のみ）で
adopt され note_in が cascade 除去されることを確認。

## E2 PPQN 拒否の境界

**仮説**: `Captured` payload を持つ branch を「編集中
candidate」と「adopt 時に凍結された旧 adopted branch」に
区別して拒否範囲を絞れるか。

- 調査: `BranchContentRef::Captured` のフィールド
  （payload_digest/scope_digest/schema_version/revision）に
  frozen/candidate の識別子はない。frozen branch も
  `adopt_branch` で再 adopt 可能で、旧 grid の musical 値を
  transplant する同一リスクを持つ。
- 案 A: 全 `Captured` を拒否（採用）— 区別不能なものを
  推定で分けない fail-closed。
- 案 B: frozen のみ除外 — 識別子がなく schema 変更が必要。
  却下。

**観察**: `cb_arr_14` で fork 直後（candidate = Captured）の
拒否と、discard 後の再射影成功を確認。adopt 後は旧 adopted
branch が `Captured` に凍結されるため、adopt 履歴のある set は
実質 PPQN 変更不可 — 意図的制約として ADR/設計に記録。

## E3 absolute anchor tempo-map policy

**仮説**: absolute anchor/extent clip を tempo map 変換で
musical scope 所有に拡張できるか。

- 調査: cb-07 exp E3 の却下理由を再確認 — musical range は
  beat/tick 座標、absolute anchor は実時間座標。clip extent の
  musical 換算は tempo 曲線全域に依存し、partial overlap の
  帰属・tempo 変化跨ぎの policy が未確立。`tempo.rs` の
  TickTimeMapping は存在するが、所有判定への適用 policy は
  sv#19/sv#20 の時間正本設計に未確立。
- 案 A: tempo map 変換で所有拡張 — policy 未確立のまま推定
  実装すると fingerprint/conflict の正本性を損なう。却下。
- 案 B: `AbsoluteAnchor` fail-closed 継続（採用）— 既存 pin
  （cb_arr_04/05）が契約を固定し続ける。

**観察**: `scope.rs`/`arrangement.rs` 無変更。cb_arr_04/05 が
非回帰で pass — `AbsoluteAnchor` boundary 報告と
`absolute_anchor: Unsupported` capability が維持される。

## E4 render 解決の追従性

**仮説**: `render_input.rs` は `plan_adopt_writes` を再利用する
ためコード変更なしで materialize に追従するか。

- 調査: `resolve_render_project` は `plan_adopt_writes` +
  `entity::write_all`（clone 上）+ `validate_references` の
  構成。`Removed` write は同一経路で適用され、cascade 漏れは
  oracle が typed error で検出。
- 観察: cb_arr_11 で `resolve_render_project` が削除済み
  composed view を返すことを確認 — render_input.rs 無変更。
