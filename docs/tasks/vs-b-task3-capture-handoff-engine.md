---
id: "TASK-oPdRML5zQpCplnqKl8f5E8"
type: "task"
status: "done"
title: "VS-B task3 capture handoff engine"
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
    - "docs/tasks/vs-b-task2-gate-schedule.md"
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

# VS-B task3 capture handoff engine

## Work

`capture.rs`（CaptureChunk/CaptureBlockHeader/CaptureSpanRecord/Validity）、`handoff.rs`（pool・filled/free SPSC・FaultLatch bit）、`engine.rs`（CaptureEngine::process の RT 経路・epoch/frame 連続性・gate 評価・PCM copy・retained 退避・Fault latch・schedule_mut・teardown）を実装。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-b-clock-gate-rt-capture.md
- docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md

## Dependencies

- task2（GateSchedule/evaluate_gate）

## Affected Files

- crates/synvelis-recording/src/{capture,handoff,engine,lib}.rs

## Done When

- [ ] process が全 fault 経路で latch+fail-stop し、失敗 chunk を retained へ退避、gate 閉のみ block で chunk を消費しない

## Verification

- [ ] `mise run check`・`mise run lint`・engine 単体系テスト緑
