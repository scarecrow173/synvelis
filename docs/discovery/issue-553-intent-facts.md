---
id: "DISC-553IntentFacts01"
type: "discovery"
status: "resolved"
title: "Issue #553 discovery — variant intent_facts の inner op facts 伝播 gap（cascade fail-closed 不全）"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/553"
    - "https://github.com/scarecrow173/synvelis/issues/550"
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
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-553-intent-facts.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
---

# Issue #553 discovery — variant intent_facts の inner op facts 伝播 gap

## 背景

synvelis#550（PR `synvelis#551`）の review で実証された
systemic な設計 gap。`send.remove` の `op_facts` は
send-owned `ModulationSource`/`ModulationAssignment` が
cascade 削除される場合に `unsupported` を立てて
fail-closed（SEC-14）するが、top-level prepare でのみ
効き、variant-edit（inner op）経路では同じ deny が
伝播しなかった。synvelis#550 では send.remove に限り
境界側で封じ込め済みで、本 Issue は伝播層
（`variant.rs` の `declared_facts`）の修正である。

## 調査結果

### 1. gap の実体は `declared_facts` の Edit arm に局在する

`crates/synvelis-gateway/src/variant.rs`
`declared_facts`（:665 以降）の
`VariantIntent::Edit` arm は
`crate::prepare::op_facts(op, project)` を呼び、
`inner.reads` と `inner.impacted` のみを外側 `OpFacts`
へ写す。返却 `OpFacts` は `preconditions: Vec::new()`・
`unsupported: Vec::new()` で構築されるため、inner の
fail-closed 情報がここで欠落する。

`provisional_gate`（`prepare_engine.rs`）は
`facts.unsupported` 非空で `PreparationRejected`
（"unverifiable effects"）を返す — 伝播すれば
top-level と同一の deny が variant-edit に効く。

### 2. 影響面は全 cascade op に及ぶが、現行で非空に
なり得るのは `RemoveSend` のみ

`lower_for`（`prepare.rs`）の capability 表で
`BranchEditOp::Project` として受理され得る inner op は
全て `op_facts` に専用 arm を持ち、`_` catch-all
（常に `unsupported` を立てる arm）には到達しない。
`unsupported` が非空になり得るのは `RemoveSend` arm の
条件付き検査（send-owned modulation entities の存在時）
と `Composite`（batch は `lower_variant` が拒否するため
inner op になり得ない）のみ。`preconditions` は全 op が
宣言するため、伝播は record 面でも意味を持つ
（changeset digest に inner の named precondition が
正直に載る）。

### 3. `preconditions` は gate ではなく record

`OpFacts.preconditions` は `PreparedOperation` →
`PreparedChangeSet` → digest に serialize されるのみで、
commit 側で再評価される gate ではない。伝播による
behavioral 変化は `unsupported` の fail-closed deny のみ
であり、preconditions の追加は proposal record の
正直さの改善に留まる。

### 4. `Adopt` arm と `BranchEditOp::Comp` は範囲外

`VariantIntent::Adopt` arm は inner facts を持たない
構造で、post-simulation diff も send-owned
`ModulationSource`/`Assignment` を観測不能と見られる
ため、adopt 経路に同等の不可視削除 gap が残る可能性が
ある（未検証・explore 時点で uncertain）。本 slice では
扱わず、実証的発見があれば記録に留める。
`BranchEditOp::Comp` inner は wire lowering で受理
されない（`prepare_engine.rs` の variant-op/batch
拒否）ため対象外。

## 決定

- `declared_facts` の `VariantIntent::Edit` arm で
  `BranchEditOp::Project(op)` の inner `OpFacts` の
  `preconditions`・`unsupported` を外側 `OpFacts` へ
  そのまま伝播する（verbatim — fail-closed 情報を
  落とさないことが要件であり、variant-edit 文脈の
  装飾は deny reason の正直さを損なわない範囲で不要）。
- reads/impacted の既存伝播（adopted branch 判定で
  impacted の扱いが変わる部分）の意味論は変更しない。
- 検証は `cb06_strip_gateway.rs` に modulated send
  fixture（send-owned `Macro` source 付き・head rev 6）
  の seed/boot helper を test-local に mirror し、
  widest honest mask（`sends` 宣言済み）で fork →
  candidate `send.remove` edit が `PreparationRejected`
  （"unverifiable"）になることを pin する。mask
  gating ではなく facts 伝播が deny 原因であることを
  分離する。

## 非目標

- `VariantIntent::Adopt` 経路の同等 gap の修正
  （未検証・別 Issue 候補）。
- `BranchEditOp::Comp` inner op の facts 伝播
  （wire lowering で受理されない）。
- 個別 op の cascade ロジック（`op_facts` 各 arm・
  capability 表・`lower_for`/`op_table`）の見直し。
- `crates/synvelis-project/**` の変更（`OpFacts` の
  send 意味論は synvelis#550 で確定済み・読み取りのみ）。
