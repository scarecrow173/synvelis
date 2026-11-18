---
id: "TSPEC-559AdoptUnobservableRemoval06"
type: "test-spec"
status: "approved"
title: "Issue #559 test-spec — adopt 観測不能 entity 削除 fail-closed の検証仕様"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/559"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
    - "docs/designs/issue-559-adopt-unobservable-removal.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/tasks/issue-559-adopt-unobservable-removal.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-559-adopt-unobservable-removal.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #559 test-spec — adopt 観測不能 entity 削除 fail-closed の検証仕様

## Feature

stored candidate payload の観測不能 entity 削除に対する
adopt fail-closed（synvelis#559）の検証。

## Rules

### Rule 1 — domain deny pin（`cb06_strip_fields.rs`）

`cb_strip_559_adopt_unobservable_removal_rejected`
（`crates/synvelis-project/tests/cb06_strip_fields.rs`）
が以下を保証する:

- send-owned modulation closure（`ModulationSource`
  owner=Send・`ParameterTarget` owner=Send/role=
  SendLevel・`ModulationAssignment` source→target）を
  持つ send に対し、`{Sends}` mask fork → domain
  `edit_branch` で `ProjectOp::RemoveSend` を適用した
  candidate が、send・edge・target・source・assignment
  を全て欠く payload として store に永続化される
  （pre-553 stored payload と同型 — domain edit gate
  `check_strip_fields_scope_removals` は受理する）。
- 当該 candidate の `try_adopt` が
  `Err(VariantError::UnverifiableRemoval { .. })` を
  返す — `Unsupported`/`AmbiguousLineage` 等の別
  discriminant ではない。
- 拒絶後の live は不変: send・send-owned source・
  SendLevel target・assignment が全て残存し、
  project revision・variant set の adopted branch も
  変化しない。

意図: `plan_adopt_writes` が観測モデル外 kind の
`Removed` write を計画する前に fail-closed する
契約と、拒絶が live/state を一切変更しないことを
固定する。

### Rule 2 — gateway deny pin（`cb06_strip_gateway.rs`）

`cb06_strip_559_adopt_unobservable_removal_denied`
（`crates/synvelis-gateway/tests/cb06_strip_gateway.rs`）
が以下を保証する:

- fixture: gain_target が send-owned `SendLevel`
  target にバインド済みの send + send-owned
  `ModulationSource` + 当該 target 宛
  `ModulationAssignment` を持つ project。
- payload 生成は **domain 層の variant API**
  （`fork_variant_set` + `edit_branch_op` に
  `ProjectOp::RemoveSend`）で rig の shared
  `BranchContentStore` に対して行う — cascade が
  send・edge・level target・modulation source・
  assignment を candidate payload から削除し、
  観測不能 `ModulationAssignment`/`ModulationSource`
  削除入り `BranchContentRef::Captured` payload が
  永続化される。post-sv#561 では wire の candidate
  edit 経路が同 cascade を `op_facts` unverifiable
  として prepare で正直に拒否するため、wire op では
  当該 payload を新規生成不能 — durable stored
  payload への防御層としての adopt fail-closed を
  pin する本来の意図に合致する注入経路。
- live project（variant set・`Captured` ref・
  payload digest を含む）は seed 済み状態で
  `CommitGate::open` し、当該 candidate の adopt
  prepare（agent + grant 経路）が
  `GatewayError::PreparationRejected` で拒絶され、
  reason が `unverifiable-removal` を含む。
- 拒絶後の live は不変: send の `gain_target` binding
  ・target・assignment が残存し、branch は adopt
  されない。

意図: durable stored payload（pre-553 wire が
永続化し得た形状と同型）が gateway 境界で stable
reason 付きに fail-closed することを end-to-end で
固定する。

### Rule 3 — 非回帰（既存 pin の再実測）

以下が引き続き Pass する:

- `cb_strip_550_send_removal_closure_and_adopt`
  （domain — 観測可能 kind の send teardown adopt
  が物質化: `RoutingConnection` の `Removed` write
  は受理維持）
- `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown`（gateway — 同上の wire
  end-to-end pin、provenance `objects == 3`）
- `cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied` /
  `cb06_strip_550_send_remove_unobservable_cascade_
  denied`（edit/live-apply の unverifiable cascade
  deny — 非接触の確認）
- 両 crate の `mise run test` 全体

### Rule 4 — 再現 evidence（文書）

- `evidence/sv559/reproduction.md` が pre-fix
  materialization の実測（手順・結果・toolchain）を
  記録し、post-fix の期待 verdict
  （`VariantError::UnverifiableRemoval`）を明記する。

## Examples

- positive（deny）: Rule 1/2 の test が Pass —
  観測不能 `Removed` write を含む candidate adopt
  が typed rejection で live 不変。
- positive（受理維持）: Rule 3 — modulation entity
  を持たない send の teardown adopt は従来どおり
  `AdoptOutcome::Applied` で物質化。

## 非対象

- 実 PCM/実 plugin/実 GUI lane（POLICIES.md §5 —
  headless のみ）。
- `op_facts` の `SetSendGainTarget`/`RemoveTrack`
  unverifiable cascade coverage — edit/live-apply
  経路の別 Issue（`synvelis#561`）領域。
  `synvelis#561` は着地済みで当該 wire 経路は
  既に正直に拒否される — Rule 2 はその拒否を
  迂回せず、domain 層で生成した durable payload が
  adopt で拒絶されることのみを pin する。
- stored payload bytes の直接捏造
  （digest 偽装・`schema_version` 偽装・手組み
  JSON 等）— `decode_candidate`/`verified_candidate`
  の integrity 契約を迂回する経路は対象外。
  domain `edit_branch_op` が生成する実 payload で
  十分であり、integrity を破る注入は別問題。

## 関連リンク

- `synvelis#559`（本 Issue）/ `synvelis#557` /
  `synvelis#553` / `synvelis#550` / `synvelis#561` /
  `synvelis#283`
- `docs/specs/issue-559-adopt-unobservable-removal.md`
- `docs/designs/issue-559-adopt-unobservable-removal.md`
- `docs/test-specs/cb-06-track-bus-adapter.md`
- `docs/tasks/issue-559-adopt-unobservable-removal.md`
- `docs/impl/ir/issue-559-adopt-unobservable-removal.md`
- `crates/synvelis-project/tests/cb06_strip_fields.rs`
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
- `POLICIES.md`
- `AGENTS.md`
