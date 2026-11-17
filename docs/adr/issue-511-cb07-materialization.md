---
id: "ADR-sv511gH8iJ0kL1mN3oP5qR7s"
type: "adr"
status: "accepted"
title: "Issue #511 ADR — Removed materialization・PPQN typed 拒否・AbsoluteAnchor 継続の方式決定"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/specs/issue-511-cb07-materialization.md"
    - "docs/designs/issue-511-cb07-materialization.md"
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
  related:
    - "docs/adr/cb-00-creative-branch.md"
    - "docs/impl/ir/cb-07-arrangement-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-511-cb07-materialization.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-511-cb07-materialization.md"
  references:
    - "docs/impl/exp/cb-07-arrangement-adapter.md"
  defers: []
  deferred-by: []
---

# Issue #511 ADR — CB-07 materialization 残件の方式決定

## Context

CB-07（synvelis#284）の 3 残件について、実装か fail-closed 維持かの
二者択一が課された。前提: sv#513（apply op surface）と sv#517
（永続 BranchContentStore + RuntimePublish）は main 統合済み。

## Decision 1: adopt/render の scope-removal を `Removed` write で materialize

`plan_adopt_writes` が計画する write に `ChangeKind::Removed` を
導入し、`entity::write_entity` に delete arm（`remove_entity`）を
追加する。materialize 対象は `entity::removal_supported_kind`
（`Clip | Note`）に限定し、他の scope-owned kind は従来どおり
typed `Unsupported`。VCS `apply_selected` の `change_supported`
は変更しない（adopt 経路限定 — sv#513 の共有 surface を再利用
せず分裂もさせない）。

- entity が candidate に実在する場合は move-out として
  `Unsupported`（adapter edit-time 判定と同一 verdict）。
- Clip 削除は `ProjectOp::RemoveClip` と同一の cascade
  （lane membership 除去 + clip-local notes 削除）を mirror —
  clips-only mask でも note は親を失えない参照整合であり、
  scope 拡大ではない。
- render 解決は同一 plan を再利用するため自動追従。
  `validate_references` が最終 oracle で半 materialize を
  構造的に排除する。

却下案: VCS apply surface まで `Removed` を拡張 — diff/removal
cascade は sv#513 の別設計であり slice 横断の再設計になる。

## Decision 2: `Captured` payload 存在時の `SetProjectPpqn` を typed 拒否

`reproject_ppqn` が、いずれかの variant set の branch が
`BranchContentRef::Captured` を保持する場合 `Error::Validation`
で拒否する。`validate`/`apply`/`preview` の 3 経路が同一関数を
通るため判定は authoritative boundary 一箇所。

- `Captured` は編集中 candidate と adopt 時に凍結された旧
  adopted branch の双方 — frozen 履歴も再 adopt 可能で同じ
  stale-grid リスクを持つため区別しない。
- 帰結: adopt 履歴のある set は実質 PPQN 変更不可。variant set
  解消後のみ変更可能 — 意図的制約として記録。

却下案: candidate payload の再射影 — decode→変換→再 encode→
新 digest→`Captured` ref/scope_digest 更新が必要で、frozen 履歴も
対象になるため slice 規模を超える。後続 slice の検討事項。

## Decision 3: absolute anchor/extent は `AbsoluteAnchor` fail-closed 継続

tempo-map 変換による musical scope 所有拡張は行わない。
absolute↔musical 変換は tempo map の versioned 解釈を要し、
clip extent の musical 換算は tempo 曲線全域に依存する。
partial overlap 帰属・tempo 変化跨ぎの policy が未確立のまま
所有に含めると fingerprint/conflict の正本性を損なう推定に
なる（cb-07 exp E3 の却下理由を踏襲）。`scope.rs`/
`arrangement.rs` は無変更で、既存 pin が契約を固定し続ける。

## Consequences

- 削除を含む Arrangement candidate が 1 CAS/Undo で adopt 可能に
  なる。`cb_arr_11`/`cb_arr_08` の fail-closed pin は新契約の
  assert へ書き換え（削除・無効化なし）。
- PPQN 変更は `Captured` payload が残る限り typed 拒否 —
  `cb_arr_14` が新契約を pin。
- absolute anchor の扱いは不変 — cb_arr_04/05 が継続して pin。
- docs index（`docs/**/README.md`）は本 slice では再生成しない
  — Coordinator が統合時に再生成する。
