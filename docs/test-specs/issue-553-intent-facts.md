---
id: "TSPEC-553IntentFacts07"
type: "test-spec"
status: "approved"
title: "Issue #553 test-spec — intent_facts 伝播の検証仕様"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/553"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-553-intent-facts.md"
    - "docs/designs/issue-553-intent-facts.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-553-intent-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/tasks/issue-553-intent-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-553-intent-facts.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-553-intent-facts.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #553 test-spec — intent_facts 伝播の検証仕様

## 検証対象

`crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
の `cb06_strip_553_variant_edit_send_remove_
unobservable_cascade_denied`（新規）と、既存の
top-level deny pin・positive variant-edit 群の非回帰。

## Rules

- R1: send-owned `ModulationSource` を持つ send への
  `send.remove` が variant-edit（candidate branch）
  として prepare されると、`PreparationRejected`
  （reason に "unverifiable"）で拒否される — inner
  `op_facts` の `unsupported` が `declared_facts` 経由で
  `provisional_gate` の fail-closed deny に届く
- R2: R1 の検証は widest honest mask（`sends` 宣言
  済み `strip-fields` scope）で行い、deny 原因が
  mask gating（`variant-unsupported`）ではなく facts
  伝播であることを構造的に分離する
- R3: 拒否は prepare 段で完結し、branch content
  revision（fork 直後の 1）・live revision・対象
  entity を一切動かさない
- R4: top-level prepare の同一 deny
  （`cb06_strip_550_send_remove_unobservable_cascade_
  denied`）と `tests/authorization.rs` の
  `sec14_unverifiable_effects_denied` が引き続き Pass
  — top-level `PreparedMutation::Project` arm は非接触
- R5: `unsupported` が空の inner op を持つ既存の
  positive variant-edit（mask-gated edit・send wire
  lowering・lifecycle の adopt/discard/query 面）が
  引き続き受理・commit される — preconditions の
  伝播は record 面の追加であり gate を変えない

## Examples

- `cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied` — modulated send
  fixture（head rev 6）で fork → candidate `send.remove`
  が "unverifiable" で `PreparationRejected`、
  branch revision 不変
- `cb06_strip_550_send_remove_unobservable_cascade_
  denied` — top-level 経路の deny pin（回帰確認）
- `cb06_strip_550_send_ops_wire_lowering` — 非
  modulated send での send 4 op positive edit
  （非回帰確認 — `unsupported` 空の inner op は
  従来通り受理）
- `cb06_strip_fields_mask_gated_edit_wire_lowering`・
  `cb06_strip_fields_insert_ops_wire_lowering` —
  channel/inserts 系 positive edit の非回帰確認

## Oracle 種別

headless（`mise run test synvelis-gateway`）。実
PCM/RT/plugin・実 GUI lane は `NotRun`（外部 gate
責務）。
