---
id: "PLAN-Qxb0ntXA0MZiwf2piEFji"
type: "plan"
status: "approved"
title: "PC-C5 slice1 実施計画 — Relation Inspector 投影（UI-C5-01・headless）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c5-01-relation-inspector.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
    - "docs/adr/pc-c5-01-relation-readmodel-coverage.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c5-01-relation-inspector.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice1 実施計画

## 前提確認

- `synvelis_project::relation`（`RelationIndex`/`RelationQuery`/
  `RelationReply`/`RelationCoverage`/`RelationState`/`RelationKind`/
  `ControlEntityRef`/`RelationDirection`）は crate root から
  re-export 済み（PC-C2, synvelis#99）。
- `ProjectSession::project()`/`revision()`/`apply()` は既存。
- `arrangement_ui.rs` → `MainScreenState` → `screens.rs` Table、
  `publish_project_length` refresh、`ui_ops::capture_screen_state`
  の `inspector.target` 捕捉は確立済み。
- `validate_references`（apply 後 oracle）は dangling control 参照を
  reject するため、ops 経由で作れる `Unresolved` は enabled-graph
  cycle のみ。dangling は PC-C2 側 test が既に担保。

## Steps

1. `crates/synvelis-workspace/src/relation_ui.rs` を新規作成
   （`RelationRow`/`RelationView`/`RelationViewOptions`/
   `RelationNav`/`relation_rows`/`relation_rows_with`）。
2. `crates/synvelis-workspace/src/lib.rs` に
   `pub mod relation_ui;` と re-export を additive に追加。
3. `crates/synvelis-app/src/screens.rs` に
   `RelationScreenState`・`MainScreenState.relations`・
   `relation_label`/`relation_table`・Inspector tab への追加。
4. `crates/synvelis-app/src/app.rs` に `relation_rows()`
   accessor・private `relation_view` helper・
   `publish_project_length` の refresh 行を additive に追加。
5. `crates/synvelis-workspace/tests/rel_c5.rs` を新規作成し、
   双方向 nav・roundtrip・Stale/Unavailable/Truncated/
   include_disabled/Unresolved(cycle)・非破壊を検証。
6. `mise run fmt-check` → `mise run check` → `mise run lint` →
   `mise run test` を実行し、失敗は実修正で潰す。
7. `git fetch origin main && git merge origin main` の後、
   最終検証を再実行。docs index を再生成。
8. impl record（`docs/impl/ir/`）を作成し、task status を
   `done` に更新。論理単位で commit → `feature/102-relation-ui`
   を push → PR 作成（`Refs #102`、Close しない）。

## Verification

| 項目 | 方法 |
| --- | --- |
| fmt | `mise run fmt-check` |
| 型 | `mise run check` |
| lint | `mise run lint`（`-D warnings`） |
| test | `mise run test`（`rel_c5.rs` を含む workspace 全体） |
| 実 GUI/OS | **NotRun**（#114/#103 gate、別 Issue） |

## Risks

- `publish_project_length` の refresh は `inspector.target` が
  未捕捉の間は空の `RelationScreenState` を書く — 「anchor なし」
  であり relation 0 件の誤表示ではない（label は "no target"
  相当を表示）。
- `RelationIndex::build` は query 毎 O(N)。publish 駆動の
  non-RT 経路のみで、現状規模では問題なし。

## Sources and Relations

- [spec](../specs/pc-c5-01-relation-inspector.md) /
  [ADR](../adr/pc-c5-01-relation-readmodel-coverage.md) /
  [design](../designs/pc-c5-01-relation-inspector-design.md) /
  [task](../tasks/pc-c5-01-relation-inspector.md) /
  [test-spec](../test-specs/pc-c5-01-relation-inspector.md)
