---
id: "TASK-3x4WFhrRf9r9qJ29dKDIfg"
type: "task"
status: "done"
title: "VS-B task5 poc acceptance tests"
created: "2026-11-02"
updated: "2026-11-02"
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
    - "docs/plans/vs-b-clock-gate-rt-capture.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vs-b-clock-gate-rt-capture.md"
    - "docs/tasks/vs-b-task4-mock-driver-writer.md"
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
    - "docs/test-specs/vs-b-e2e-r1-mock-gate-rt-capture.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-B task5 poc acceptance tests

## Work

`recording-poc` に VS-B シナリオ（Loop 3 pass + punch 区間を含む決定的実行）を追加し stdout summary を出力。test-spec 対応表の全テスト（rt_capture/rt_fault/rt_safety/rt_contract/plan/poc_determinism 更新）を実装。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-b-clock-gate-rt-capture.md
- docs/test-specs/vs-b-e2e-r1-mock-gate-rt-capture.md

## Dependencies

- task4（mock driver）

## Affected Files

- crates/recording-poc/src/main.rs
- crates/recording-poc/tests/poc_determinism.rs
- crates/synvelis-recording/tests/{rt_capture,rt_fault,rt_safety,rt_contract,plan}.rs

## Done When

- [ ] spec AC 全項目のテストが緑、poc 2 回実行同一出力

## Verification

- [ ] `mise run test`・`mise run lint`・`mise run fmt-check` 全緑
