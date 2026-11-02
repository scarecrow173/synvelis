---
id: "TASK-HVCTNeUo2kXQN0NvHCUa6K"
type: "task"
status: "done"
title: "MD-A task2 Comp スキーマ (ID/registry/POD) + CompOp + 検証"
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
    - "docs/tasks/mda-task1-normalized-placement-schema.md"
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

# MD-A task2 Comp スキーマ + CompOp + 検証

## Work

- `id.rs`: `TakeGroupId`/`CompId`/`CompRegionId`/`CompTransitionId`。
- `comp.rs` (新): `TakeGroup`/`Comp`/`CompRegion`/`CompTransition`/
  `CompCandidateRef`/`FadeShape`/`CompRegistry` POD。
- `project.rs`: `comp: CompRegistry` + `validate_references` に
  comp 参照整合 (owner・adjacency・overlap・handle・非重複)。
- `comp.rs`: `CompOp` (`Operation` impl) — group/comp/region/transition
  の全編集 op。

## Done

- CompOp が controller 経路で validate→atomic apply され、
  MD-A-02 の各拒否ケースが `Error` で返る。
