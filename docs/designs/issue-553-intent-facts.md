---
id: "DESIGN-553IntentFacts03"
type: "design"
status: "approved"
title: "Issue #553 design — declared_facts の inner facts 伝播"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/553"
    - "docs/specs/issue-553-intent-facts.md"
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
    - "docs/specs/issue-553-intent-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-553-intent-facts.md"
    - "crates/synvelis-gateway/src/variant.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
---

# Issue #553 design — declared_facts の inner facts 伝播

## 全体構造

修正は `crates/synvelis-gateway/src/variant.rs` の
`declared_facts` に閉じる。認可・fail-closed の責務境界
は既存のまま:

- **`prepare::op_facts`（facts 権威）**: 各 `ProjectOp`
  の reads/preconditions/unsupported を pinned project
  に対して宣言する。synvelis#550 で `RemoveSend` の
  条件付き cascade 検査が実装済み。
- **`variant::declared_facts`（variant intent の facts
  合成）**: variant intent 固有の reads/impacted
  （set・branch・scope roots・inner op の read/impact
  集合）を合成し、inner op の preconditions/unsupported
  を透過させる。
- **`prepare_engine::provisional_gate`（deny 権威）**:
  `facts.unsupported` 非空 → `PreparationRejected`
  （"unverifiable effects"）。この gate は
  `PreparedMutation::Project`・`Variant`・`Render` の
  全経路で同一に適用される — 伝播層を直せば deny は
  variant-edit にも top-level と同一になる。

## 変更点

`declared_facts` 内に `preconditions`・`unsupported`
の accumulator を追加し、`VariantIntent::Edit` arm の
`BranchEditOp::Project(op)` 分岐で inner `OpFacts` の
同名 field を `extend` する。最終 `OpFacts` 構築は
accumulator をそのまま使う（Fork/Adopt/Discard arm は
inner op を持たず空のまま）。

```text
VariantIntent::Edit { intent }
  └─ BranchEditOp::Project(op)
       └─ inner = prepare::op_facts(op, project)
            ├─ reads      → reads.extend        （既存）
            ├─ impacted   → adopted? impacted : reads  （既存）
            ├─ preconditions → preconditions.extend  （新規）
            └─ unsupported   → unsupported.extend    （新規）
```

`BranchEditOp::Comp` は `if let` が match しないため
inner facts を持たず、従来通り空のまま — wire
lowering が Comp inner を受理しないため到達不能で
あり、将来 Comp 経路が wire に載る場合は別 gap として
扱う。

## 設計判断

- **verbatim 伝播を選択**: inner の
  `preconditions`/`unsupported` をそのまま外側へ移す。
  `Precondition.code` は `&'static str` で variant-edit
  文脈を付加する余地がなく、`unsupported` の文字列に
  "variant-edit" prefix を付けると top-level と deny
  reason が乖離する。`declared_facts` の結果に
  `unsupported` が存在する時点でそれは必ず variant
  edit の inner op 由来（Fork/Adopt/Discard は空）
  なので、文脈は構造的に自明 — 装飾は冗長。
- **preconditions の伝播も行う**: gate ではなく
  changeset record だが、Issue の受入条件が
  "unsupported / preconditions を正しく伝播" を
  要求しており、inner op の named precondition が
  proposal record に正直に載ることは fail-closed
  契約の一部（何を根拠に合法と判断したかの記録）。
- **facts の評価対象は pinned base のまま**:
  `op_facts(op, project)` は `pin.state`（base）に
  対して計算される — reads/impacted と同じ既存の
  近似を維持する。candidate branch の内容が base と
  乖離していても、fail-closed 判定は base の事実に
  基づく保守側に倒れる（base に send-owned source
  があれば deny — candidate で既に除去済みでも
  deny し得るが、これは pinned-base facts の
  既存 semantics であり本 slice で変えない）。

## テスト設計

`cb06_strip_gateway.rs` に synvelis#550 の modulated
fixture（send-owned `Macro` source 付き・head rev 6）
の seed/boot helper を test-local に mirror し、
widest honest mask（`strip_scope()` — `sends` 宣言
済み）で fork → candidate `send.remove` edit を
`expect_edit_rejected`（"unverifiable"）で pin する。
mask が `sends` を宣言しているため、deny は
`StripFieldsAdapter` の mask gate ではなく
`provisional_gate` の facts 伝播が原因であることが
構造的に分離される。branch content revision が
fork 直後の 1 のままであることも assert し、
reject が何も mutate しないことを pin する。

## リスク

- **preconditions の changeset digest 変化**: variant
  edit の changeset に inner preconditions が載る
  より digest が変わるが、digest は prepare 毎に
  新規計算されるため pinned 値との不整合は起きない。
  既存 test が全て Pass することで非回帰を実測する。
- **過剰 deny の可能性**: base に残る send-owned
  source により、candidate 側で既に除去済みの
  send.remove も deny し得る — pinned-base facts の
  fail-closed semantics として許容（top-level も
  同じ base 評価）。
