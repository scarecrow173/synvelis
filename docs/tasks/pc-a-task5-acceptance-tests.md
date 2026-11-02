---
id: "TASK-034PCA09Up4dXy0eGhUiJk"
type: "task"
status: "done"
title: "PC-A task5 受入 test PC-A-01..08"
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
    - "docs/plans/pc-a-parameter-control.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-a-parameter-control.md"
    - "docs/tasks/pc-a-task4-control-session.md"
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
    - "docs/test-specs/pc-a-e2e-parameter-control.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-A task5 受入 test PC-A-01..08

## Work

- `tests/pc_a.rs`: test-spec 準拠の受入 test。単一正本/3 投影、
  scope 分離・clip 追従、競合検出/Reject/Replace、breakdown + 観測
  状態、mode 状態機械 + 二重適用防止 + 1 undo + loop seal +
  fixation、DAG cycle 拒否・順序独立、capability (readonly/
  not-automatable/quantized/means/非線形)、時間整合 (tempo/beat/
  clip/loop/offline=playback/stamp→frame)。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/pc-a-parameter-control.md
- docs/test-specs/pc-a-e2e-parameter-control.md

## Dependencies

- docs/tasks/pc-a-task4-control-session.md

## Verification

- `mise run test` PASS (`pc_a` 全 test green)。
