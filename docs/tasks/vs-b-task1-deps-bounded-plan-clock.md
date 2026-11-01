---
id: "TASK-YUqbpBWfyNBWBrdnL1eX38"
type: "task"
status: "done"
title: "VS-B task1 deps bounded plan clock"
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

# VS-B task1 deps bounded plan clock

## Work

`ruviform-rt` git 依存と dev-dep `stats_alloc` を追加し `Cargo.lock` を再生成。`bounded.rs`（BoundedVec）、`plan.rs`（RecordingRequest/CaptureTarget/GateSpec/BufferBudget/PreparedRecordingPlan/preflight 検証）、`clock.rs`（InputBlock・epoch/frame 連続性チェッカ）を実装。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-b-clock-gate-rt-capture.md
- docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md

## Dependencies

- 直列依存（plan Task Breakdown 参照）

## Affected Files

- Cargo.toml / Cargo.lock
- crates/synvelis-recording/Cargo.toml
- crates/synvelis-recording/src/{bounded,plan,clock,lib}.rs

## Done When

- [ ] preflight が不正 request を全件拒否（不存在 lane/epoch・逆転・容量 0・tempo compile 不可）し、`mise run check`/`lint` が緑

## Verification

- [ ] `mise run check`・`mise run lint`・`tests/plan.rs` 緑
