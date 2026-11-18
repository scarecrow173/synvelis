---
id: "IR-553IntentFacts08"
type: "impl"
status: "completed"
title: "Issue #553 impl IR — intent_facts 伝播 実装記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/553"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-553-intent-facts.md"
      - type: "file"
        path: "docs/specs/issue-553-intent-facts.md"
      - type: "file"
        path: "docs/designs/issue-553-intent-facts.md"
      - type: "file"
        path: "docs/plans/issue-553-intent-facts.md"
      - type: "file"
        path: "docs/tasks/issue-553-intent-facts.md"
      - type: "file"
        path: "docs/test-specs/issue-553-intent-facts.md"
      - type: "file"
        path: "docs/impl/exp/issue-553-intent-facts.md"
      - type: "dir"
        path: "evidence/sv553/"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/variant.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-553-intent-facts.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-553-intent-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-553-intent-facts.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-553-intent-facts.md"
    rejected: []
---

# Issue #553 impl IR — intent_facts 伝播 実装記録

## 実装サマリ

`variant.rs` の `declared_facts` が `VariantIntent::Edit`
の inner `ProjectOp` に対して `op_facts` を呼びながら
`preconditions`/`unsupported` を破棄していた gap を修正
した。Edit arm の `BranchEditOp::Project` 分岐で inner
`OpFacts` の同名 field を外側 `OpFacts` へ verbatim
伝播し、`provisional_gate` の "unverifiable effects"
fail-closed deny が variant-edit 経路にも top-level と
同一に効くようにした。synvelis#550 review で実証され
IR に記録されていた残存 gap（`docs/impl/ir/
issue-550-strip-routing-ops.md` の review 対応節）の
解消である。

## 変更内容

### `crates/synvelis-gateway/src/variant.rs`

- `declared_facts` に `preconditions`・`unsupported`
  の accumulator（`Vec`）を追加。
- `VariantIntent::Edit` arm の `BranchEditOp::Project`
  分岐で、inner `OpFacts` の `preconditions`・
  `unsupported` を `extend` で外側へ伝播。
  `reads`/`impacted` の既存伝播（adopted branch
  判定で impacted の扱いが変わる部分）は不変。
- 最終 `OpFacts` 構築を accumulator 参照へ変更
  （Fork/Adopt/Discard arm は inner op を持たず空の
  まま）。
- doc comment に inner facts の verbatim 伝播契約と
  synvelis#553 の根拠を追記。

`BranchEditOp::Comp` は `if let` が match しないため
inner facts を持たない — wire lowering が Comp inner
を受理しないため到達不能（将来 Comp 経路が wire に
載る場合は別 gap）。

### `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`

- `seed_strip_send_modulated_store`・
  `boot_strip_send_modulated` を追加 — synvelis#550
  の modulated fixture（send-owned `Macro` source
  付き・head rev 6）の seed/boot を `seed_strip_send_
  store`/`boot_strip_send` の mirror として test-local
  に実装（`rig/mod.rs` 共有 fixture は非接触）。
- `cb06_strip_553_variant_edit_send_remove_unobservable_
  cascade_denied` を追加 — widest honest mask
  （`strip_scope()`・`sends` 宣言済み）で fork →
  candidate `send.remove` edit が `PreparationRejected`
  （"unverifiable"）で拒否され、branch content
  revision が fork 直後の 1 のままであることを pin。

### docs/evidence

- issue-553 doc chain 8 type を `issue-553-intent-
  facts` slug で作成。generated index（`docs/**/
  README.md`）は未編集 — Coordinator が統合時に
  再生成。
- cb-06 doc 3 件（test-specs/impl ir/impl exp）に
  synvelis#553 の追補を追加。
- `evidence/sv553/verification.md` を新設。

## 検証

`mise run` 系の canonical 検証と結果は
`evidence/sv553/verification.md` に記録。cargo 直接
実行は補助証拠（canonical = mise）。

- red 確認: 修正前に新規 test を実行し、variant-edit
  `send.remove` の prepare が `Completed` で受理
  されること（= inner `unsupported` が落ちていた
  こと）を実測。
- green 確認: 修正後に `cb06_strip_gateway` 全 8 件
  Pass — 新規 deny test・top-level pin
  `cb06_strip_550_send_remove_unobservable_cascade_
  denied`・positive variant-edit 群の非回帰を含む。

## review 対応

`review-protocol` を 1 回実施（cooperative/adversarial
の 2 pass + candidate 検証）。blocker 0・verdict
ACCEPT。確認した候補と判定:

- inner `preconditions` の changeset/digest への影響
  → record 面の追加のみで gate 非該当、全 test Pass
  で非回帰を実測（REJECTED）。
- `unsupported` 伝播による過剰 deny → lowerable
  inner op の `op_facts` arm を棚卸しし、非空に
  なり得るのは `RemoveSend`（send-owned modulation
  entities 存在時）のみと確認 — 意図した fail-closed
  であり他 op の verdict は不変（REJECTED）。
- test が正しい deny 経路を pin しているか →
  "unverifiable" は `provisional_gate` の unsupported
  check のみが生成し、widest mask で mask gating を
  分離、branch revision assert で無変更を確認
  （CONFIRMED・問題なし）。
- `VariantIntent::Adopt` の同等 gap → 実在の可能性
  あるが Issue/lease 範囲外。EXP E4 と PR「未対応の
  作業」に記録（Non-blocking）。
