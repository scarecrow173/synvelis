---
id: "PLAN-7zXphX8skvXfNgeXarocfo"
type: "plan"
status: "approved"
title: "PC-C6 oracle REL/TR headless fixture 実装計画"
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
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  verified-by:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 oracle REL/TR headless fixture 実装計画 Implementation Plan

## Goal

`synvelis-project` に REL01〜07 / TR01〜08 の headless oracle fixture
test を追加し、merge 済み契約本体（`RelationIndex`/`ControlTrace`）を
独立算出の期待値と照合する。`src/`・他 crate・Cargo 設定は変更しない。

## Scope

- In: `crates/synvelis-project/tests/pc_c6_oracle_rel.rs`・
  `pc_c6_oracle_tr.rs`（新規）、docs チェーン、impl record、
  生成 index。
- Out: spec の Out of Scope と同一。

## Upstream Documents

- spec: `docs/specs/pc-c6-oracle-rel-tr.md`
- adr: `docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md`
- design: `docs/designs/pc-c6-oracle-rel-tr-design.md`
- test-spec: `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md`

## Design Inputs

- FixtureModel（宣言的 control モデル）+ oracle 導出（edge/state/BFS）
  + `assert_oracle_agrees` 照合（design doc が正本）。
- TR 側は `oracle_freshness`/`oracle_channel`/`oracle_coherence` の
  独立分類実装。
- 先例: `tests/pc_c2_relation.rs`（direct control map 構築・fixture
  helper）、`tests/trace_c3.rs`（合成 `MediaTime`/receipt/stamp）。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-project/tests/pc_c6_oracle_rel.rs` | 新規 — REL01〜07 oracle fixture |
| `crates/synvelis-project/tests/pc_c6_oracle_tr.rs` | 新規 — TR01〜08 oracle fixture |
| `docs/impl/ir/pc-c6-oracle-rel-tr.md` | 実装記録（完了時） |
| `docs/*/README.md` | 生成 index 再生成 |

## Implementation Sequence

1. REL fixture 基盤: `FixtureModel` + oracle 導出（edges/state/query）
   + `assert_oracle_agrees`。
2. REL01〜REL06 の Case 実装（方向・source 種別・複数 assignment・
   disabled・cycle・bounds・stale/unavailable）。
3. REL07 巨大 graph fixture + `Instant` 実測 + oracle 一致確認。
4. TR fixture 基盤 + oracle 分類実装、TR01〜TR08 実装。
5. `mise run fmt-check` / `check` / `lint` / `test`（canonical）。
6. impl record + docs index 再生成 + commit。

## Dependencies and Risks

- 契約本体は変更しない — oracle と実装の差異が出た場合は契約解釈を
  spec/design/test-spec に立ち返って確認する（実装側を oracle に
  合わせて変更することはしない）。
- `Instant` 計測は判定に使わない（証拠記録のみ）。
- 巨大 fixture は test 時間を監視し、数十秒を超える場合は規模を
  見直す。

## Task Breakdown

- T1: REL oracle 基盤 + REL01〜REL06
- T2: REL07 巨大 graph 実測
- T3: TR oracle 基盤 + TR01〜TR08
- T4: 最終検証 + impl record + index 再生成

## Verification

- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` — 全 exit status を impl record に記録。
- test-spec `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md` の各 Feature
  が fixture で担保されること。

## Open Questions / Gaps

- なし（review-protocol で blocker なし確認済み）。

## Self-Review

- scope は spec の In Scope と一致。`src/`・他 crate・Cargo 設定への
  変更なし。oracle 独立性は design/ADR の決定どおり。

## Implementation Handoff

- task: `docs/tasks/pc-c6-task1-oracle-rel-tr.md`

## Review Handoff

- review-protocol 実施済み（blocker なし）。
