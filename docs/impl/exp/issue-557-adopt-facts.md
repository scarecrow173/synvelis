---
id: "EXP-557AdoptFacts08"
type: "experiment"
status: "completed"
title: "Issue #557 impl EXP — adopt declared_facts 監査 実験記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/issue-557-adopt-facts.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-553-intent-facts.md"
    - "docs/impl/exp/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-557-adopt-facts.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #557 impl EXP — adopt declared_facts 監査 実験記録

## 実験サマリ

代替実装の実験は実施しなかった — 監査の結論は
「gap 無し」であり、production code の変更自体が
不要と判定されたため。採否の判断は全て spec/design
段階で確定し、実装は既存 seam（`declared_facts`・
`variant::simulate`・post-simulation `derive_effects`/
`authorize_effects`・`plan_adopt_writes`）の消費のみ。

## 採否記録（exp.jsonl と対応）

| EXP | 仮説 | 結果 | 要点 |
| --- | --- | --- | --- |
| EXP-1 | `AdoptIntent` が inner op を持ち `op_facts` 伝播が必要 | rejected | `AdoptIntent` は set/branch/CAS/approval のみで inner op 非所持。`lower_adopt` も CAS 4 項目のみ受理 — 伝播すべき inner facts は型レベルで存在しない |
| EXP-2 | provisional `impacted` に scope roots が無いのは facts-drop の穴 | rejected | prepare は post-simulation `derive_effects` + `authorize_effects` が実 diff を認可する二層構造。scope write は実 diff で覆われる — 設計どおり |
| EXP-3 | 現行 wire で不可視削除入り candidate を生成可能 | rejected | 観測不能 kind の cascade を持つ wire op は send.remove のみで、send-owned modulation entities 存在時は synvelis#553 以降 edit deny。modulation 系/connection 系 op は wire capability 非存在 |
| EXP-4 | pre-553 stored candidate payload の adopt は fail-closed で拒絶される | rejected | `plan_adopt_writes` は `removal_supported_kind`（sv#550 cascade 物質化設計）で観測不能 kind を `Removed` write 化し、post-simulation gate を素通り — hardening は `synvelis#559` に委譲 |
| EXP-5 | deny pin（不可視削除入り candidate の adopt 拒絶）を test 追加 | rejected | AC2 は「gap がある場合」の条件付きで gap 無しのため条件不成立。受理側 pin `cb06_strip_557_*` を代わりに追加 — stored payload 再現は payload 捏造が必要で `synvelis#559` の hardening とセットで扱うのが適切 |
| EXP-6 | adopt 受理契約を pin test で固定 | adopted | `cb06_strip_557_adopt_send_remove_candidate_materializes_teardown` — fork→candidate send.remove→adopt で teardown（objects==3）を実 diff 経由で物質化することを headless 実測 |

## 実測

- `cb06_strip_557_adopt_send_remove_candidate_
  materializes_teardown`: Pass — provenance
  `branch == candidate`・`outcome == "applied"`・
  `objects == 3`、live から send config/`Send` edge/
  send-owned `SendLevel` target が除去。
- `cb06_strip_gateway` 全 9 件 Pass — 新規 pin +
  既存 `cb06_strip_553_*`/`cb06_strip_550_*` の
  非回帰。
- gateway/project の check/lint/test・fmt-check・
  check-headless-deps・prek 全て exit 0
  （`evidence/sv557/verification.md`）。

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#559` / `synvelis#283`
- `docs/impl/ir/issue-557-adopt-facts.md`
- `docs/impl/exp/issue-557-adopt-facts-exp.jsonl`
- `docs/impl/exp/issue-553-intent-facts.md`
- `docs/impl/exp/cb-06-track-bus-adapter.md`
- `evidence/sv557/`
- `POLICIES.md`
