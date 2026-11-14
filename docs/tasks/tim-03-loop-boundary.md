---
id: "TASK-034XzQStcKCOQBnq9rBjtq"
type: "task"
status: "in-progress"
title: "TIM-03 Loop/Tempo boundary 自動導出 — 実装タスク"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-03-loop-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/plans/tim-03-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-03-loop-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-03-loop-boundary.md"
  derived-by: []
  verifies:
    - "docs/test-specs/tim-03-loop-boundary.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  defers: []
  deferred-by: []
---

# TIM-03 Loop/Tempo boundary 自動導出 — 実装タスク

## Work

`WrapSim`（`process_boundary.rs`、`NodeSurface` が
維持）+ `boundary_loop_wraps`（`routing_control.rs`）
から upcoming window 内の loop wrap `BoundaryPoint` を
非 RT で導出し、
`tim03_loop_boundary.rs` で実 scheduler との一致を
検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/tim-03-loop-boundary.md
- docs/designs/tim-03-loop-boundary.md
- docs/specs/tim-03-loop-boundary.md
- docs/test-specs/tim-03-loop-boundary.md

## Dependencies

- docs/plans/tim-03-loop-boundary.md
- TIM-03 session→driver/routing wiring（PR #308、統合済み）

## Affected Files

- `crates/synvelis-player/src/process_boundary.rs`
- `crates/synvelis-player/src/routing_control.rs`
- `crates/synvelis-player/src/dispatch.rs`（必要時のみ —
  公開 re-export 経路の確認。原則変更なし）
- `crates/synvelis-player/tests/tim03_loop_boundary.rs`（新規）
- `docs/impl/ir/tim-03-loop-boundary.md`（新規）
- `docs/impl/exp/tim-03-loop-boundary.md`（新規）
- `evidence/sv127/`（新規）
- `docs/*/README.md`（生成 index、再生成のみ）

## Done When

- [ ] `WrapSim::walk_window` が scheduler と同一
  algebra（pos_at/next_wrap/wrap/apply tie 規則）で
  window 内 wrap offset を導出し、`NodeSurface`
  が観測 window ごとに sim を進める
- [ ] `boundary_loop_wraps` が surface/provenance ゲート
  + wrap_sim clone + scheduled/staged merge を行い
  typed error を返す
- [ ] `tim03_loop_boundary.rs` が test-spec の
  fixture matrix（E1–E9 相当）を網羅し、実 scheduler
  wrap frame と一致を検証する
- [ ] `mise run fmt-check`/`check synvelis-player`/
  `lint synvelis-player`/`test synvelis-player` が
  pass
- [ ] impl/ir・impl/exp 文書と evidence/sv127/ が
  作成され、docs index が再生成されている

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check synvelis-player`
- [ ] `mise run lint synvelis-player`
- [ ] `mise run test synvelis-player`
- [ ] review-protocol レビューで blocker なし
