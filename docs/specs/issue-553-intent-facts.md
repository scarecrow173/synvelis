---
id: "SPEC-553IntentFacts02"
type: "spec"
status: "approved"
title: "Issue #553 spec — variant intent_facts の inner op facts 伝播"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/553"
    - "docs/discovery/issue-553-intent-facts.md"
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
    - "docs/discovery/issue-553-intent-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-553-intent-facts.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
---

# Issue #553 spec — variant intent_facts の inner op facts 伝播

## 目的

`variant.rs` の `declared_facts` が `VariantIntent::Edit`
の inner `ProjectOp` に対して `op_facts` を呼びながら
`preconditions`/`unsupported` を破棄していた gap を修正し、
variant-edit 経路でも inner op の fail-closed verdict
（SEC-14 "unverifiable effects"）が top-level prepare と
同一に適用されることを保証する。CB-06/548/550 で構築した
variant-edit wire 経路の認可・fail-closed 契約を
cascade op 全般へ拡張する。

## 要件

### R1 — inner facts の伝播

- `declared_facts` の `VariantIntent::Edit` arm で、
  `BranchEditOp::Project(op)` の inner `OpFacts` が持つ
  `preconditions`・`unsupported` を外側 `OpFacts` へ
  伝播する。
- 伝播は verbatim とし、fail-closed 情報
  （`unsupported` の非空判定が `provisional_gate` の
  "unverifiable effects" deny に直結する）を欠落
  させない。variant-edit 文脈の detail 装飾は行わない
  （deny reason は inner op の facts をそのまま報告する
  のが top-level と同一の正直さ）。
- `reads`/`impacted` の既存伝播ロジック（adopted
  branch 判定で impacted の扱いが変わる部分）の
  意味論は変更しない。
- `VariantIntent::Fork`/`Adopt`/`Discard` arm の
  facts は変更しない（inner op を持たない構造）。

### R2 — variant-edit 経路の fail-closed 実測

- send-owned `ModulationSource` を持つ send に対する
  `send.remove` の variant-edit（candidate branch）が
  `PreparationRejected`（reason に "unverifiable"）で
  拒否されることを headless で実測する。
- 検証は widest honest mask（`sends` 宣言済み
  `strip-fields` scope）で行い、deny 原因が mask
  gating ではなく facts 伝播であることを分離する。
- 拒否は prepare 段で完結し、branch content revision・
  live revision・対象 entity を一切動かさない。

### R3 — top-level deny 経路の非回帰

- top-level prepare の `send.remove` deny
  （`cb06_strip_550_send_remove_unobservable_cascade_
  denied`）と `tests/authorization.rs` の
  `sec14_unverifiable_effects_denied` が引き続き Pass
  すること。
- 既存の positive variant-edit（mask-gated edit・
  send wire lowering 各 test）が引き続き受理・commit
  されること — `unsupported` が空の inner op の
  挙動は不変。

## 受入条件

- `variant.rs` の `intent_facts`/`declared_facts` が
  inner op の `unsupported`/`preconditions` を正しく
  伝播する。
- variant-edit 経路で send.remove の cascade
  fail-closed（send-owned 削除 deny）が伝播し、
  不可視削除が受理されないことを headless で実測。
- top-level prepare の deny 経路（"unverifiable
  effects"）に回帰がない。

## 非目標

- `VariantIntent::Adopt` 経路の同等 gap の修正
  （post-simulation diff が send-owned modulation
  entities を観測不能と見られる未検証事項 — 実証的
  発見があれば記録のみ）。
- `BranchEditOp::Comp` inner op の facts 伝播
  （wire lowering で受理されない）。
- 個別 op の cascade ロジック・`op_facts` 各 arm・
  wire capability 表（`lower_for`/`op_table`）の
  見直し — 本 Issue は伝播層の修正のみ。
- `crates/synvelis-project/**` の変更。
