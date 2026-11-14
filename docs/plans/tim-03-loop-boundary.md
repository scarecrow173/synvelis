---
id: "PLAN-034XzPAftHuisduKCPsMLt"
type: "plan"
status: "approved"
title: "TIM-03 Loop/Tempo boundary 自動導出 — 実装計画"
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
    - "docs/specs/tim-03-loop-boundary.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-03-loop-boundary.md"
    - "docs/test-specs/tim-03-loop-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-loop-boundary.md"
    - "docs/designs/tim-03-loop-boundary.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-03-loop-boundary.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  defers: []
  deferred-by: []
---

# TIM-03 Loop/Tempo boundary 自動導出 — 実装計画 Implementation Plan

## Goal

`TransportMirror` の commanded state と scheduled/staged
transport commands から、upcoming callback window 内の
loop wrap `BoundaryPoint{LoopWrap}` 列を非 RT で自動導出し、
process boundary oracle へ供給可能にする。

## Scope

1 実装 stream。spec `docs/specs/tim-03-loop-boundary.md` /
design `docs/designs/tim-03-loop-boundary.md` の範囲のみ。

## Upstream Documents

- docs/specs/tim-03-loop-boundary.md
- docs/designs/tim-03-loop-boundary.md
- docs/specs/tim03-process-boundary.md（消費する既存契約）
- docs/specs/tim-03-session-driver-wiring.md（mirror 配線元）

## Design Inputs

- docs/designs/tim-03-loop-boundary.md

## File Map

- `crates/synvelis-player/src/process_boundary.rs` —
  `pub(crate)` scheduler 等価 scalar state `WrapSim`
  （pos_at/next_wrap/wrap/apply_command/walk_window）
  を追加。
- `crates/synvelis-player/src/routing_control.rs` —
  `NodeSurface.wrap_sim`（vend 時初期化、
  `observe_node_block` が pending 適用列で walk
  advance）と `RoutingControlDispatch::boundary_loop_wraps`
  を追加（surface/provenance ゲート + wrap_sim
  clone + scheduled/staged merge + walk_window）。
- `crates/synvelis-player/tests/tim03_loop_boundary.rs` —
  新規 test file。Fake Plugin + 実 `RoutingControlDispatch`
  経路で wrap offset と実 scheduler wrap frame の一致を
  検証。
- `docs/impl/ir/tim-03-loop-boundary.md` /
  `docs/impl/exp/tim-03-loop-boundary.md` — 実装記録・
  実験記録。
- `evidence/sv127/` — 検証証拠（テスト出力・check 結果）。
- `docs/*/README.md`（生成 index、`doc-maintenance` で
  再生成）。

## Implementation Sequence

- [ ] `process_boundary.rs` に `WrapSim`
  （`walk_window` 含む）を実装（TDD: 先に unit-level で
  wrap algebra を検証可能な形にし、integration test
  と併走）。
- [ ] `routing_control.rs` に `NodeSurface.wrap_sim`
  維持 + `boundary_loop_wraps` を実装
  （`observe_node_block` の scheduled/staged 解決規則を
  再利用し、sim は clone して preview walk）。
- [ ] `tests/tim03_loop_boundary.rs` を作成し、
  test-spec の fixture matrix を実装。
- [ ] `mise run fmt-check` / `check` / `lint` / `test`
  `synvelis-player` を実行し pass を確認。
- [ ] impl/ir・impl/exp 文書を作成、docs index を再生成、
  `evidence/sv127/` に検証証拠を保存。
- [ ] review-protocol レビュー → blocker 解消 →
  commit/push/PR。

## Dependencies and Risks

- `TransportMirror`/`TransportScheduler`（driver.rs・
  Ruviform）は読み取り専用で消費。変更しない。
- `WrapSim` は mirror とは独立した scheduler 等価
  scalar state — mirror の commanded-state
  approximation（scheduler reject・out-of-band
  `control()` を見ない既知 limitation）を継承し、
  wrap 導出も同 approximation 上に立つことを文書化済み。
- staged `due_frames` の絶対解決は「`boundary_loop_wraps`
  の call_start == 次の observe_block call_start」契約に
  依存 — doc comment で明記。
- `position_base == frame_base`（1 tick == 1 engine frame）
  の frame-linear domain 前提。tempo map は musical 情報
  専用で wrap frame を動かさない不変条件をコード・文書に
  明記。

## Task Breakdown

- 1 task（`docs/tasks/tim-03-loop-boundary.md`）: 上記
  sequence を一括実施。fixture matrix は test-spec 参照。

## Verification

- [ ] `mise run fmt-check` — pass
- [ ] `mise run check synvelis-player` — pass
- [ ] `mise run lint synvelis-player` — pass
- [ ] `mise run test synvelis-player` — pass（新規 test
  file を含む）
- [ ] 導出 offset が実 scheduler の wrap frame
  （plugin `TransportInfo` position fold）と一致
- [ ] `boundary_plan_window`/`boundary_stage_window` への
  供給合成で oracle legality 検査が動作

## Open Questions / Gaps

- wrap が window 先頭 frame（offset 0）に来る場合の
  oracle 契約は head apply と同一（cut にならない）—
  既存 `command_boundary`/`window_offset` 規則に従う。
- plan/tempo install が transport scalar state を変えない
  不変条件は fixture で検証（wrap 位置不変）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology
  are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker が直接実装）
- Independent work streams: none
- User approval required before dispatch: yes
  （review-protocol レビューで代替 — contract 準拠）
- If approved: discover available implementation and
  delegation capabilities in the current environment, then
  execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- reviewer は spec/design の Out of Scope 遵守・
  `WrapSim::walk_window`/`boundary_loop_wraps` の
  scheduler algebra 一致性・
  test fixture の網羅性を重点確認。
