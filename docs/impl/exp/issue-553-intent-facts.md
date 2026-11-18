---
id: "EXP-553IntentFacts09"
type: "experiment"
status: "completed"
title: "Issue #553 impl EXP — intent_facts 伝播 実験記録"
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
    - "docs/impl/ir/issue-553-intent-facts.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-553-intent-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-550-strip-routing-ops.md"
    - "docs/impl/exp/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-553-intent-facts.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #553 impl EXP — intent_facts 伝播 実験記録

## E1: gap の再現 — red 状態の実測

- **試行**: `cb06_strip_553_variant_edit_send_remove_
  unobservable_cascade_denied` を修正前コードで実行。
- **観測**: `expect_edit_rejected` が panic —
  variant-edit `send.remove`（candidate branch、
  widest mask）の prepare が `Completed` で受理された。
  inner `op_facts` の `unsupported`
  （"send-owned modulation entities are outside the
  observable model"）が `declared_facts` の返却
  `OpFacts`（`unsupported: Vec::new()`）で破棄され、
  `provisional_gate` の deny に届かなかったことを
  実測で確認。
- **結論**: Issue 記述どおりの伝播 gap が variant-edit
  経路に存在。top-level pin
  （`cb06_strip_550_send_remove_unobservable_cascade_
  denied`）は同一 fixture で Pass しており、差分は
  伝播層のみに局在する。

## E2: 伝播の形 — verbatim vs 文脈装飾

- **検討**: inner の `preconditions`/`unsupported`
  をそのまま外側へ移す案と、"variant-edit inner
  op:" 等の detail/prefix で包む案。
- **決定**: verbatim。`Precondition.code` は
  `&'static str` で variant 文脈を付加する型的余地が
  なく、`unsupported` 文字列への prefix は top-level
  と deny reason を乖離させる。`declared_facts` の
  結果に `unsupported` が存在する時点で variant
  edit inner 由来は構造的に自明（Fork/Adopt/
  Discard は空）なため装飾は冗長 — KISS。
- **副次確認**: `preconditions` は `PreparedChangeSet`
  → digest に serialize される record であり、
  commit 側で再評価される gate ではない
  （`commit.rs`/`proposal_engine.rs` に評価経路
  なし）。伝播による behavioral 変化は
  `unsupported` の fail-closed deny のみ。

## E3: inner op の unsupported 到達可能性の棚卸し

- **調査**: `lower_for` capability 表の全 entry が
  生成し得る `ProjectOp` と `op_facts` の match arm
  を照合。`_` catch-all（常に `unsupported`）に
  到達する lowerable op は存在しない —
  `unsupported` が非空になり得るのは `RemoveSend`
  arm の条件付き検査（send-owned modulation
  entities 存在時）と `Composite`（`lower_variant`
  が batch/variant-op を inner として拒否するため
  到達不能）のみ。
- **結論**: 本修正で新たに deny され得るのは
  send-owned modulation entities を持つ send の
  `send.remove` variant-edit のみ — 他の全 inner
  op の受理/拒否 verdict は不変（`unsupported`
  空）。preconditions の伝播は record 面の追加。

## E4: Adopt 経路の同等 gap — 範囲外の記録

- **観測**: `VariantIntent::Adopt` arm は inner
  facts を持たず、post-simulation diff が
  send-owned `ModulationSource`/`Assignment` を
  観測不能と見られるため、adopt 経路に同等の
  不可視削除 gap が残る可能性がある（explore 時点
  で未検証・uncertain）。
- **対応**: 本 slice の lease・Issue 範囲外のため
  修正せず記録のみ。adopt は candidate payload の
  materialize であり、candidate 側で既に
  fail-closed 済みの edit が adopt される構造上、
  gap の実害は限定的と推測されるが未検証 — Issue
  化の判断は Coordinator。
