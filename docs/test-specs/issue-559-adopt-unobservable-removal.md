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
  `ModulationAssignment` を持つ project（head
  revision 7）。
- fork（`{Sends}` を含む widest honest mask）→
  candidate に wire 合法 op `synvelis.send.
  set-gain-target`（`gain_target: null`）を commit —
  `release_send_level_target` cascade が target・
  assignment を candidate から削除し、観測不能
  `ModulationAssignment` 削除入り payload が
  `BranchContentStore` に永続化される
  （`op_facts` が当該 arm を unverifiable と flag
  しない現行の wire 経路 — discovery §4）。
- 当該 candidate の adopt prepare（agent + grant
  経路）が `GatewayError::PreparationRejected`
  で拒絶され、reason が `variant-unverifiable-
  removal` を含む（`variant simulation failed:`
  prefix 経由の stable kebab-case reason）。
- 拒絶後の live は不変: send の `gain_target` binding
  ・target・assignment が残存し、branch は adopt
  されない。

意図: wire から到達可能な stored-payload 同型
scenario が gateway 境界で stable reason 付きに
fail-closed することを end-to-end で固定する。

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
  経路の別 Issue（`synvelis#561`）領域。Rule 2 は
  当該 gap が生成した candidate の adopt 拒絶のみ
  を pin し、gap 自体の修正は要求しない。
- stored payload bytes の直接注入
  （`BranchContentRef::Captured` 差替等）— domain
  `edit_branch`/wire 合法 op が生成する同型 payload
  で十分であり、捏造経路の再現は対象外。

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
