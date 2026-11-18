---
id: "PLAN-553IntentFacts05"
type: "plan"
status: "approved"
title: "Issue #553 plan — intent_facts 伝播 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/553"
    - "docs/specs/issue-553-intent-facts.md"
    - "docs/designs/issue-553-intent-facts.md"
  changes:
    added: []
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-553-intent-facts.md"
    - "docs/designs/issue-553-intent-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-553-intent-facts.md"
    - "docs/test-specs/issue-553-intent-facts.md"
---

# Issue #553 plan — intent_facts 伝播 slice

## 分割

単一 slice。test 追加（red 確認）→ `variant.rs` 修正
（green 確認）→ doc chain/evidence → 全体検証 →
review の順で進める。

## file map

| 領域 | file | 内容 |
| --- | --- | --- |
| 伝播修正 | `crates/synvelis-gateway/src/variant.rs` | `declared_facts` に preconditions/unsupported accumulator を追加し Edit arm の `BranchEditOp::Project` inner facts を伝播。doc comment 更新 |
| test | `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` | modulated send fixture の `seed_strip_send_modulated_store`/`boot_strip_send_modulated` helper と variant-edit deny test `cb06_strip_553_variant_edit_send_remove_unobservable_cascade_denied` を追加 |
| docs | issue-553 doc chain 8 type | discovery/spec/design/plan/tasks/test-specs/impl ir/impl exp を新設 |
| docs | cb-06 doc 3 件 | synvelis#553 の追補（stale 化した記述の追従） |
| evidence | `evidence/sv553/` | verification.md 新設 |

## リスクと rollback

- **既存 variant-edit の非回帰**: inner `unsupported`
  が空の op は挙動不変。`unsupported` が非空になり得る
  のは `RemoveSend`（send-owned modulation entities
  存在時）のみで、positive 経路を持つ既存 fixture
  （非 modulated send）には影響しない。`mise run
  test synvelis-gateway` 全件で実測。
- **preconditions の record 変化**: changeset digest
  に inner preconditions が載るが、digest は prepare
  毎の新規計算で pinned 値を持たないため非互換なし。
- **rollback**: `variant.rs` の Edit arm への
  `extend` 2 行と accumulator を戻せば完全に元の
  挙動へ戻る（test も同時に戻す）。

## verification matrix

| 層 | 手段 |
| --- | --- |
| gateway integration | `mise run test synvelis-gateway`（cb06_strip_gateway 8 件 — 新規 deny test + top-level pin + positive edit 群を含む） |
| project 非回帰 | `mise run check/lint/test synvelis-project`（Issue 検証方法の指定どおり — 本修正は project 非接触だが下流整合のため実行） |
| lint/fmt | `mise run lint synvelis-gateway`・`mise run fmt-check` |
| headless deps | `mise run check-headless-deps` |
| hooks | `mise exec -- prek run --all-files` |
