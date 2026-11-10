---
id: "TASK-034VmVPWtNFIg5zO3xaDsP"
type: "task"
status: "in-progress"
title: "PC-C6: R01〜R08 / PL01〜PL08 / UI01〜UI07 / COMP01〜COMP05 headless oracle fixture 実装タスク"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/plans/pc-c6-oracle-rows-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c6-oracle-rows-plan.md"
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/designs/pc-c6-oracle-rows-design.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c6-oracle-rows-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-oracle-rows.md"
  verified-by:
    - "docs/test-specs/pc-c6-oracle-rows.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6: R01〜R08 / PL01〜PL08 / UI01〜UI07 / COMP01〜COMP05 headless oracle fixture 実装タスク

## Work

- T1 — `crates/synvelis-player/tests/pc_c6_oracle_pl.rs`（新規）:
  Rig 流用 + PL01〜PL08。`pc_c6_oracle_pl_rt.rs`（新規）: PL05
  RT noalloc（stats_alloc・独立 binary）。
- T2 — `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`（新規）:
  OracleModel + R01〜R08。
- T3 — `crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs`（新規）:
  宣言 edge 集合 + UI01〜UI07。
- T4 — 最終検証（`git fetch origin main && git merge origin/main` →
  `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`）
  + COMP01〜05 証拠分類 + impl record + docs index 再生成。

## Classification

test-only slice / synvelis-workspace + synvelis-player 層 headless
oracle（Issue #103 の PC-C1-R01〜08・PC-C4-PL01〜08・
PC-C5-UI01〜07・PC-C6-COMP01〜05 行。synvelis-workspace 追加は
Coordinator 承認済み `msg_7802648feb93`）

## Upstream Documents

- plan: `docs/plans/pc-c6-oracle-rows-plan.md`
- spec: `docs/specs/pc-c6-oracle-rows.md`
- design: `docs/designs/pc-c6-oracle-rows-design.md`
- adr: `docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md`
- test-spec: `docs/test-specs/pc-c6-oracle-rows.md`

## Dependencies

- 前提: `0f15294` 以降の `origin/main`（PR #212 REL/TR・PC-C1/C4/C5
  契約本体 merge 済み）。
- 参照のみ: `crates/synvelis-workspace/src/`（control/recording/
  relation_ui — 変更禁止）、`crates/synvelis-player/src/`（binding/
  dispatch/runtime — 変更禁止）、既存 test（fixture 流儀の先例・
  変更しない）。

## Affected Files

- `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`（新規）
- `crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs`（新規）
- `crates/synvelis-player/tests/pc_c6_oracle_pl.rs`（新規）
- `crates/synvelis-player/tests/pc_c6_oracle_pl_rt.rs`（新規）
- `docs/` チェーン + `docs/impl/ir/` + 生成 index

## Done When

- [ ] R01〜R08・PL01〜PL08・UI01〜UI07 が新規 oracle fixture test で
  独立導出の期待値と照合され全 pass
- [ ] PL05 RT noalloc が stats_alloc で実測・記録
- [ ] COMP01〜05 の headless 実施可能部分が Pass/Fail/NotRun/
  Unsupported で分類記録
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`
  が全て exit 0
- [ ] impl record + index 再生成済み、残件が NotRun/Deferred 記録

## Verification

- canonical: `mise run fmt-check` / `mise run check` /
  `mise run lint` / `mise run test` / `mise run test-rt` /
  `mise run test-msrv`
- 補助証拠: 直接 cargo 実行結果（canonical = mise）
