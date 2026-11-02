---
id: "TASK-jlbvNYC9NvQW1NsWa4wo4K"
type: "task"
status: "done"
title: "MD-A task3 Effective Comp 投影 + editor target 型"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
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
    - "docs/plans/mda-comp-model-poc.md"
  implemented-by: []
  depends-on:
    - "docs/plans/mda-comp-model-poc.md"
    - "docs/tasks/mda-task2-comp-schema-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/mda-model-poc-acceptance.md"
  references: []
  defers: []
  deferred-by: []
---

# MD-A task3 Effective Comp 投影 + editor target 型

## Work

- `comp.rs`: `project_comp` 純粋導出、`EffectiveComp`/
  `EffectiveSegment`/`EffectiveTransition`/`SourceBinding`/
  `CompProvenance`、`segment_at`/`transition_at` 位置問合せ。
- `comp.rs` or `editor.rs`: `EditorDisplayTarget`/
  `EditorEditingTarget`/`SourceVersionRef` (version 0 = raw 拒否)。
- handle 導出: take coverage (segment source_range union) /
  existing-source window / silence。

## Done

- 同一 Project からの投影が完全一致・Source 不変・位置問合せが
  高々 1 件を返す。
