---
id: "TASK-IOg16XR9hFXm4HxznkaIlM"
type: "task"
status: "done"
title: "VS-B task2 gate schedule"
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
    - "docs/tasks/vs-b-task1-deps-bounded-plan-clock.md"
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

# VS-B task2 gate schedule

## Work

`gate.rs` を実装: `GateSpan`（tempo 一様分割済み半開 span + pass_index + project 区間）、`GateSchedule`（有界 window + scheduled_until + 消費）、`GateScheduler`（tick→frame 変換・tempo 境界分割・Loop/PunchInLoop 展開・`extend` refill）、`evaluate_gate`（block 半開交差→CapturePiece、underrun 検出）。gate 単体テストを追加。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-b-clock-gate-rt-capture.md
- docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md

## Dependencies

- task1（bounded・plan の SessionFrame/TickTimeMapping）

## Affected Files

- crates/synvelis-recording/src/gate.rs
- crates/synvelis-recording/src/lib.rs
- crates/synvelis-recording/tests/gate.rs（任意・task5 に統合可）

## Done When

- [ ] Normal/Punch/Loop/PunchInLoop の span 生成が半開・非重複・tempo 一様分割され、evaluate_gate が mid-block 境界を一意分類、underrun を検出

## Verification

- [ ] `mise run check`・`mise run lint`・gate 系テスト緑
