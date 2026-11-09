---
id: "TASK-wl7Q7EP5HpNVOP8NAdEB7a"
type: "task"
status: "in-progress"
title: "PC-C6: REL01〜07 / TR01〜08 headless oracle fixture 実装タスク"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c6-oracle-rel-tr.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c6-oracle-rel-tr.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c6-oracle-rel-tr.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  verified-by:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6: REL01〜07 / TR01〜08 headless oracle fixture 実装タスク

## Work

- T1 — `crates/synvelis-project/tests/pc_c6_oracle_rel.rs`（新規）:
  `FixtureModel` + oracle 導出（edges/state/BFS）+ `assert_oracle_agrees`
  + REL01〜REL06。
- T2 — 同ファイル: REL07 巨大 graph fixture（数万 edge 級）+
  `Instant` 実測記録 + oracle 一致確認。
- T3 — `crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（新規）:
  oracle 分類実装 + TR01〜TR08。
- T4 — 最終検証（`git fetch origin main && git merge origin/main` →
  `mise run fmt-check`/`check`/`lint`/`test`）+ impl record +
  docs index 再生成。

## Classification

- test-only slice / synvelis-project 層 headless oracle
  （Issue #103 の PC-C2-REL01〜07・PC-C3-TR01〜08 行）

## Upstream Documents

- plan: `docs/plans/pc-c6-oracle-rel-tr.md`
- spec: `docs/specs/pc-c6-oracle-rel-tr.md`
- design: `docs/designs/pc-c6-oracle-rel-tr-design.md`
- adr: `docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md`
- test-spec: `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md`

## Dependencies

- 前提: `de70404` 以降の `origin/main`（PR #204 relation・#206 trace
  merge 済み）。
- 参照のみ: `crates/synvelis-project/src/relation.rs`・
  `src/trace.rs`（契約本体・変更禁止）、`tests/pc_c2_relation.rs`・
  `tests/trace_c3.rs`（fixture 流儀の先例・変更しない）。

## Affected Files

- `crates/synvelis-project/tests/pc_c6_oracle_rel.rs`（新規）
- `crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（新規）
- `docs/` チェーン + `docs/impl/ir/` + 生成 index

## Done When

- [ ] REL01〜07・TR01〜08 が新規 oracle fixture test で独立算出の
  期待値と照合され全 pass
- [ ] REL07 の巨大 graph 実測値が test output と impl record に記録
- [ ] `mise run fmt-check`/`check`/`lint`/`test` が全て exit 0
- [ ] impl record + index 再生成済み、残件が NotRun/Deferred 記録

## Verification

- canonical: `mise run fmt-check` / `mise run check` /
  `mise run lint` / `mise run test`
- 補助証拠: 直接 cargo 実行結果（canonical = mise）
