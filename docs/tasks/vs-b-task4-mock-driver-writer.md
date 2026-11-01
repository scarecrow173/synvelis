---
id: "TASK-R3hI9GxjhaO5En49GPl1YK"
type: "task"
status: "done"
title: "VS-B task4 mock driver writer"
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
    - "docs/tasks/vs-b-task3-capture-handoff-engine.md"
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

# VS-B task4 mock driver writer

## Work

`mock.rs` を実装: `MockPcm`（frame/channel/seed → f32 の純粋関数）、`MockDriver`（固定/変動 block 列生成・refill タイミング・pump インターリーブ）、`InspectionWriter`（filled drain・記録・free 返却・lost/double-return fault injection）、`accounting`（退役時 chunk 総数照合）。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-b-clock-gate-rt-capture.md
- docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md

## Dependencies

- task3（engine/handoff/capture 契約）

## Affected Files

- crates/synvelis-recording/src/{mock,lib}.rs

## Done When

- [ ] 単一 stream と multi-track（同 clock）の pump 駆動が決定的に動き、fault injection と退役照合が機能する

## Verification

- [ ] `mise run check`・`mise run lint`・driver 系テスト緑
