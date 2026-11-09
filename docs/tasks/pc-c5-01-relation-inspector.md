---
id: "TASK-WK8a2J24wszr2UBRAjVZe"
type: "task"
status: "in-progress"
title: "PC-C5 slice1 タスク分解 — Relation Index の Inspector 投影（UI-C5-01・headless）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c5-01-relation-inspector.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c5-01-relation-inspector.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
    - "docs/test-specs/pc-c5-01-relation-inspector.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c5-01-relation-inspector.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice1 タスク

| # | タスク | 完了条件 |
| --- | --- | --- |
| 1 | `relation_ui.rs` + `lib.rs` export | `RelationRow`/`RelationView`/`RelationViewOptions`/`RelationNav`/`relation_rows`/`relation_rows_with` が compile し、verbatim coverage/state を運ぶ |
| 2 | `screens.rs` Inspector relation 表示 | `RelationScreenState` + 第 2 `Table` が spec に現れ、coverage/state が表示される |
| 3 | `app.rs` accessor + refresh | `relation_rows()` が anchor parse→query を返し、`publish_project_length` が `screen.relations` を更新する（additive のみ） |
| 4 | `tests/rel_c5.rs` | 双方向 nav・roundtrip・Stale/Unavailable/Truncated/include_disabled/Unresolved(cycle)・非破壊が pass |
| 5 | canonical 検証 | `mise run fmt-check`/`check`/`lint`/`test` が pass |
| 6 | docs + impl record + PR | 文書チェーン完備、index 再生成、impl record、`feature/102-relation-ui` push・PR（`Refs #102`） |

## Sources and Relations

- [spec](../specs/pc-c5-01-relation-inspector.md) /
  [ADR](../adr/pc-c5-01-relation-readmodel-coverage.md) /
  [design](../designs/pc-c5-01-relation-inspector-design.md) /
  [plan](../plans/pc-c5-01-relation-inspector.md) /
  [test-spec](../test-specs/pc-c5-01-relation-inspector.md)
