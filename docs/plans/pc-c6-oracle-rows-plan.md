---
id: "PLAN-034VmVEzTuUHmBKxPRoFID"
type: "plan"
status: "approved"
title: "PC-C6 oracle R/PL/UI/COMP headless fixture 実装計画"
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
    - "docs/specs/pc-c6-oracle-rows.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-c6-oracle-rows-design.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/designs/pc-c6-oracle-rows-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-oracle-rows.md"
  verified-by:
    - "docs/test-specs/pc-c6-oracle-rows.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 oracle R/PL/UI/COMP headless fixture 実装計画 Implementation Plan

## Goal

`synvelis-workspace` / `synvelis-player` に R01〜R08・PL01〜PL08・
UI01〜UI07 の headless oracle fixture test を追加し、merge 済み
契約本体を独立算出の期待値と照合する。COMP01〜05 は headless 実施
可能部分の証拠分類を impl record に記録する。`src/`・既存 test・
manifest・他 crate は変更しない。

## Scope

- In: `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`・
  `pc_c6_oracle_ui.rs`、`crates/synvelis-player/tests/
  pc_c6_oracle_pl.rs`・`pc_c6_oracle_pl_rt.rs`（全て新規）、
  docs チェーン、impl record、生成 index。
- Out: spec の Out of Scope と同一。

## Upstream Documents

- spec: `docs/specs/pc-c6-oracle-rows.md`
- adr: `docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md`（同一
  oracle 方式の先例）
- design: `docs/designs/pc-c6-oracle-rows-design.md`
- test-spec: `docs/test-specs/pc-c6-oracle-rows.md`

## Design Inputs

- Recording 側: `OracleModel`（宣言的 fixed context）+
  `control_with` 経由の観測 + transaction field 照合
  （design doc が正本）。
- Plugin 側: `pc_c4_binding.rs` の Rig 流儀 + typed rejection/
  delivery log/queue bound 照合。RT noalloc は独立 binary。
- UI 側: 宣言 edge 集合 + `relation_rows_with`/`RelationNav`/
  `control_edge_*`/undo 深さの照合。
- 先例: `tests/pc_c1.rs`（workspace control fixture）、
  `tests/pc_c4_binding.rs`（player binding fixture）、
  `tests/uic5.rs`（workspace view/gesture fixture）、
  `tests/tim02_midi_rt.rs`（stats_alloc 独立 binary）。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs` | 新規 — R01〜R08 oracle fixture |
| `crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs` | 新規 — UI01〜UI07 oracle fixture |
| `crates/synvelis-player/tests/pc_c6_oracle_pl.rs` | 新規 — PL01〜PL08 oracle fixture |
| `crates/synvelis-player/tests/pc_c6_oracle_pl_rt.rs` | 新規 — PL05 RT noalloc |
| `docs/impl/ir/pc-c6-oracle-rows-slice.md` | 実装記録（完了時） |
| `docs/*/README.md` | 生成 index 再生成 |

## Implementation Sequence

1. PL oracle 基盤（Rig 流用）+ PL01〜PL08 + RT noalloc binary。
2. Recording oracle 基盤（OracleModel + fixture helpers）+
   R01〜R08。
3. UI oracle 基盤（宣言 edge 集合 + gesture/undo 観測）+
   UI01〜UI07。
4. `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
   `test-msrv`（canonical）。
5. COMP01〜05 の headless 実施可能部分を証拠分類として impl
   record に記録。
6. `git fetch origin main && git merge origin/main` → 生成 index
   再生成 → impl record + commit。

## Dependencies and Risks

- 契約本体は変更しない — oracle と実装の差異が出た場合は契約解釈を
  spec/design/test-spec に立ち返って確認する（実装側を oracle に
  合わせて変更することはしない）。
- `stats_alloc` は process-global — 独立 binary + window 分離を厳守。
- workspace test 追加は承認済み範囲（`msg_7802648feb93`）に限定し、
  manifest/`src/` に触れない。

## Task Breakdown

- T1: PL oracle + RT noalloc
- T2: Recording oracle R01〜R08
- T3: UI oracle UI01〜UI07
- T4: 最終検証 + COMP 証拠分類 + impl record + index 再生成

## Verification

- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` / `mise run test-rt` / `mise run test-msrv` —
  全 exit status を impl record に記録。
- test-spec `docs/test-specs/pc-c6-oracle-rows.md` の各 Feature
  が fixture で担保されること。

## Open Questions / Gaps

- なし（review-protocol で blocker なし確認済み）。

## Self-Review

- scope は spec の In Scope と一致。`src/`・他 crate・manifest への
  変更なし。workspace 追加は承認済み範囲のみ。oracle 独立性は
  design/ADR の決定どおり。

## Implementation Handoff

- task: `docs/tasks/pc-c6-task1-oracle-rows.md`

## Review Handoff

- review-protocol 実施済み（blocker なし）。
