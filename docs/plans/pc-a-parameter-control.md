---
id: "PLAN-034PCA04Pk9ySt5zBcPdEf"
type: "plan"
status: "completed"
title: "PC-A Parameter Control Model headless PoC 実装計画"
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
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/designs/pc-a-parameter-control-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-a-e2e-parameter-control.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-A Parameter Control Model headless PoC 実装計画

## Goal

Issue #40 の #6 合意 8 領域 (正本/Scope/競合/Breakdown/状態機械/Graph/
Capability/時間整合) を `synvelis-workspace` + `synvelis-project` 上に
実装し、PC-A-01..08 の headless 受入 test を全 PASS させる。

## Scope

単一実装ストリーム。境界: `crates/synvelis-project` (control model・
eval・graph・dispatch・ops・schema 2) と `crates/synvelis-workspace`
(`ControlSession` + tests)。ruviform 本体・実 adapter・GUI 経路には
触れない。

## Upstream Documents

- docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md
- docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md
- docs/designs/pc-a-parameter-control-design.md
- docs/designs/ws-a-workspace-foundation-design.md (前提基盤)

## Design Inputs

- synvelis-project: `ProjectController::apply` (validate→scratch→
  atomic→1 undo) / `ProjectOp` / `Range<F>` / `define_id!` /
  `TickTimeMapping` / `MediaTimeValue` 流 wire DTO。
- synvelis-workspace: `WorkspaceManager` / `ProjectSession`
  (apply/undo/redo + `EngineStub`) / `ViewStateRegistry` /
  `CommandContext`。
- ruviform: `plugin-core` (param/param_write/event) + `media-core`
  (BeatPosition/Ppqn/TickPosition/TempoMap/timed_input/observe)。

## File Map

- add: `crates/synvelis-project/src/control.rs` — model/eval/graph/
  dispatch/conflict
- modify: `crates/synvelis-project/src/{id,project,ops,error,store,lib}.rs`
- add: `crates/synvelis-workspace/src/control.rs` — ControlSession
- modify: `crates/synvelis-workspace/src/{session,error,lib,workspace}.rs`
- modify: root `Cargo.toml` + `crates/synvelis-project/Cargo.toml`
  (`ruviform-plugin-core`) + `Cargo.lock` (同一 rev)
- add: `crates/synvelis-workspace/tests/pc_a.rs`

## Implementation Sequence

- [x] task1: `ruviform-plugin-core` 依存追加 + control model (id/scope/
  BeatPoint/target/lane/source/assignment) + `Project::control` +
  schema 2 + `validate_references`
- [x] task2: `ProjectOp` control ops + conflict detection
  (Reject/Replace) + write/erase region
- [x] task3: eval (scope 解決/chase/modulation 合成/clamp/quantize) +
  control graph compile (cycle 拒否) + capability/dispatch
- [x] task4: `ControlSession` (mode/arm/gesture/recording/playback/
  observation/stamp/projections) + workspace API
- [x] task5: 受入 test pc_a.rs (PC-A-01..08)
- [x] task6: 最終検証 (fmt/check/lint/test) + impl record + review 記録

## Dependencies and Risks

- `ruviform-plugin-core` は同一 git rev `94c5bf8` の crate 追加のみ —
  rev 固定・ローカル再実装なし。
- `SCHEMA_VERSION` 1→2 (意図的破壊的変更、`control` は
  `#[serde(default)]`)。
- beat→f64 評価は `BeatPosition::to_f64` (documented lossy) — 正本は
  有理数で保持。

## Task Breakdown

| Task | 内容 | 依存 |
| --- | --- | --- |
| pc-a-task1 | plugin-core dep + control model + `Project::control` + schema 2 | — |
| pc-a-task2 | control `ProjectOp` + conflict detection | task1 |
| pc-a-task3 | eval + graph compile + capability/dispatch | task1 |
| pc-a-task4 | `ControlSession` + workspace API | task2, task3 |
| pc-a-task5 | 受入 test PC-A-01..08 | task4 |
| pc-a-task6 | 最終検証 + impl record | task5 |

## Verification

- [x] `mise run fmt-check` / `check` / `lint` / `test` 全 PASS。
- [x] `tests/pc_a.rs` が PC-A-01..08 を個別 test で検証。
- [x] 合意 8 領域各々に対応 test が存在。
- [ ] review-protocol で blocker なし (Coordinator pass 記録)。

## Open Questions / Gaps

- Note scope は型のみ (評価 `Unavailable`)。Clip stretch/Loop は
  1:1 写像のみ。Take multi-pass・sample-accurate・実 adapter は
  deferred。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution (dispatched worker)。
- Independent work streams: なし (単一チェーン・直列)。
- User approval required before dispatch: 承認代替として
  review-protocol を実施済み。

## Review Handoff

- `SCHEMA_VERSION` 2 で schema 1 snapshot は読めない — PoC 間の
  意図的破壊的変更として明示。
- `ruviform-plugin-core` 追加は新規 crate dep — rev 固定・
  `cargo tree` で波及確認を reviewer へ明示。
