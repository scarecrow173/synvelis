---
id: "SPEC-557AdoptFacts02"
type: "spec"
status: "approved"
title: "Issue #557 spec — VariantIntent::Adopt 経路の declared_facts 伝播監査"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
    - "docs/discovery/issue-557-adopt-facts.md"
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
    - "docs/discovery/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-557-adopt-facts.md"
    - "docs/test-specs/issue-557-adopt-facts.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
---

# Issue #557 spec — VariantIntent::Adopt 経路の declared_facts 伝播監査

## 目的

synvelis#553 で修正した Edit 経路の inner facts 伝播 gap
と同型の欠陥が `VariantIntent::Adopt` 経路に残っていない
かを監査し、(a) gap が無いことの構造的根拠を記録するか、
(b) gap が実証された場合は sv#553 と同方針で修正する。
あわせて adopt 経路の fail-closed/受理契約を pin test で
固定し、Edit 経路に回帰がないことを再実測する。

## 要件

### R1 — 監査根拠の記録（AC1）

`VariantIntent::Adopt` の `declared_facts` が inner op の
`unsupported`/`preconditions` を伝播するか、gap が無い
ことの根拠が doc chain と `evidence/sv557/` に記録される。

監査で確認すべき論点:

- `AdoptIntent` が inner op を持つか（持たなければ伝播
  すべき inner facts は構造上存在しない）。
- adopt が生成する write が prepare のどの gate で
  認可されるか（provisional `declared_facts` のみか、
  post-simulation `derive_effects` + `authorize_effects`
  が実 diff を覆うか）。
- 観測不能 entity（`ResourceKind`/`ENUMERATED_KINDS` 外）
  の削除が candidate payload に入り得る wire 経路の
  有無と、残存経路（stored payload）の評価。

### R2 — adopt 経路の pin test（AC2・条件付き）

gap がある場合、adopt 経路で cascade fail-closed が発動
することを headless で実測する test を追加する。gap が無い
場合でも、監査で特定した契約（adopt が candidate の
scope diff を実 diff 経由で物質化し post-simulation
認可を通ること）を pin する test の追加は許容される。

### R3 — Edit 経路の非回帰（AC3）

`declared_facts` Adopt arm 周辺を含む変更後も、synvelis#553
の pin（`cb06_strip_553_variant_edit_send_remove_
unobservable_cascade_denied`）と synvelis#550 の pin
（`cb06_strip_550_send_remove_unobservable_cascade_
denied`）を含む `mise run test synvelis-gateway` が
exit 0 で Pass する。

## 受入条件（Issue 転記）

- `VariantIntent::Adopt` 経路の declared_facts が inner
  op の `unsupported`/`preconditions` を伝播する（または
  gap が無いことの根拠が記録される）。
- gap がある場合、adopt 経路で cascade fail-closed が発動
  することを headless で実測する test が追加される。
- Edit 経路の修正（sv#553）に回帰がない。

## 非目標

- `crates/synvelis-project/**` の変更。
- `declared_facts` Edit arm の再修正（synvelis#553 で
  完了）。
- stored candidate payload の hardening 実装 — 監査で
  残存経路が確認された場合は別 Issue 化する
  （本 slice では `synvelis#559` として起票済み）。
- 個別 op の cascade ロジック・wire capability 表の
  見直し。
- 実 PCM/実 plugin/実 GUI lane。

## 検証方法

Issue 指定どおり:

- `mise run check synvelis-gateway` /
  `mise run lint synvelis-gateway` /
  `mise run test synvelis-gateway`
- `mise run check synvelis-project` /
  `mise run lint synvelis-project` /
  `mise run test synvelis-project`（project 非接触だが
  監査対象 domain 層の下流整合として実行）
- `mise run fmt-check` /
  `mise run check-headless-deps` /
  `mise exec -- prek run --all-files`

headless 実測のみ。実機/RT/GUI lane は NotRun。

## 関連リンク

- `synvelis#557`（本 Issue）
- `docs/discovery/issue-557-adopt-facts.md`
- `docs/designs/issue-557-adopt-facts.md`
- `docs/test-specs/issue-557-adopt-facts.md`
- `docs/test-specs/cb-06-track-bus-adapter.md`
- `synvelis#559`（stored candidate hardening）
- `synvelis#553` / `synvelis#550` / `synvelis#283`
- `docs/specs/issue-553-intent-facts.md`
- `POLICIES.md`
