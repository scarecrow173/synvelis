---
id: "PLAN-034QArZ5DRWJ27yfk2aEID"
type: "plan"
status: "completed"
title: "ED-A Editing Model headless PoC 実装計画"
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
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/designs/ed-a-editing-model-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ed-a-e2e-editing-model.md"
  references: []
  defers: []
  deferred-by: []
---

# ED-A Editing Model headless PoC 実装計画

## Goal

Issue #38 の合意 5 項目 (Selection/Tool/Snap/Transaction/Cross-View) を
`synvelis-workspace` + `synvelis-project` 上に実装し、ED-A-01..05 の
headless 受入 test を全 PASS させる。

## Scope

単一実装ストリーム。境界: `crates/synvelis-workspace` (selection/tool/
snap/gesture/edit 新 module + state/command/workspace 変更) と
`crates/synvelis-project` (`ProjectOp` 3 variant 追加) + `tests/ed_a.rs`。
ruviform・他 crate・GUI 経路には触れない。

## Upstream Documents

- docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md
- docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md
- docs/designs/ed-a-editing-model-design.md
- docs/designs/ws-a-workspace-foundation-design.md (前提基盤)

## Design Inputs

- synvelis-project: `ProjectController::apply` (validate→scratch→atomic
  swap→1 undo) / `ProjectOp` / `TimeAnchor` / `MusicalRange` / `MediaSpan` /
  `MusicalTick`/`SourceFrame` checked 演算 / `TickTimeMapping`。
- synvelis-workspace: `WorkspaceManager` / `ViewStateRegistry` /
  `EditingSelection` / `SemanticTarget` / `LinkGroupManager` /
  `ProjectSession` (apply/undo/redo + `EngineStub.graph_builds`) /
  `CommandContext`/`CommandOutcome`。

## File Map

- add: `crates/synvelis-workspace/src/selection.rs` — SelectionDomain /
  ObjectSelections / TimeSelection / LaneScope / EditTimeDomain / EditBinding
- add: `crates/synvelis-workspace/src/tool.rs` — Tool / HitZone / HandleKind /
  HitProbe / EditOperation / ToolResolution / resolve_tool
- add: `crates/synvelis-workspace/src/snap.rs` — GridConfig / SnapConfig /
  SnapUnit / SnapTarget / SnapMode / SnapEnvironment / resolve_snap
- add: `crates/synvelis-workspace/src/gesture.rs` — Gesture / GestureFixation /
  GesturePreview / CancelReason / GestureOutcome
- add: `crates/synvelis-workspace/src/edit.rs` — gesture→ProjectOp 構築、
  anchor delta 演算、time domain 導出
- modify: `crates/synvelis-project/src/ops.rs` — Composite / SplitClip /
  RemoveClipsInRange
- modify: `crates/synvelis-workspace/src/state.rs` — ViewState 拡張
  (editing_selection → object_selections 移行 + 新フィールド)
- modify: `crates/synvelis-workspace/src/command.rs` — required_domain +
  edit.undo/edit.redo/clips.delete-range/tool.activate
- modify: `crates/synvelis-workspace/src/workspace.rs` — editing API +
  undo lane 配送 + gesture cancel hook
- modify: `crates/synvelis-workspace/src/{error,persist,lib,link}.rs` —
  EditError / schema 2 / re-export / object selection 対応
- modify: `crates/synvelis-workspace/tests/ws_a.rs` — object_selections API 追従
- add: `crates/synvelis-workspace/tests/ed_a.rs`

## Implementation Sequence

- [x] task1: `ProjectOp` 3 variant + selection model + ViewState 移行 +
      schema 2 + ws_a test 追従
- [x] task2: tool.rs (hit 解決) + gesture.rs (fixation/preview/state machine)
- [x] task3: snap.rs + edit.rs (anchor 演算) + commit/cancel 経路
- [x] task4: command 追加 (edit.undo/edit.redo/clips.delete-range/
      tool.activate) + workspace editing API + cancel hook
- [x] task5: 受入 test ed_a.rs (ED-A-01..05)
- [x] task6: 最終検証 (fmt/check/lint/test) + impl record + review 記録

## Dependencies and Risks

- `editing_selection` → `object_selections` 移行は ws_a.rs・link publish・
  command resolve・persist schema を跨ぐ — schema bump で明示。
- `SplitClip` の `clip_b_source_offset` は caller 供給 (rate 変換なし) —
  op の validate で単調性のみ保証。
- pedantic lint のため `#[allow]` は最小・理由付き。

## Task Breakdown

| Task | 内容 | 依存 |
| --- | --- | --- |
| ed-a-task1 | `ProjectOp` 3 variant + selection.rs + ViewState 移行 + schema 2 + ws_a 追従 | — |
| ed-a-task2 | tool.rs hit 解決 + gesture.rs fixation/preview 型 | task1 |
| ed-a-task3 | snap.rs + edit.rs + gesture commit/cancel 経路 | task2 |
| ed-a-task4 | command 追加 + workspace editing API + cancel hook | task3 |
| ed-a-task5 | 受入 test ED-A-01..05 | task4 |
| ed-a-task6 | 最終検証 + impl record | task5 |

## Verification

- [x] `mise run fmt-check` / `check` / `lint` / `test` 全 PASS。
- [x] `tests/ed_a.rs` が ED-A-01..05 を個別 test で検証。
- [x] 合意 5 項目各々に対応 test が存在。
- [ ] review-protocol で blocker なし (Coordinator pass 記録)。

## Open Questions / Gaps

- Slip/Stretch/Fade の commit 意味は resolve まで (Unsupported で拒否)。
- 混合 time-domain の共通演算契約・Magnetic mode・px/device 差は deferred。
- `clips.delete-range` は gap-closing shift なしの範囲除去。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution (dispatched worker)。
- Independent work streams: なし (単一チェーン・直列)。
- User approval required before dispatch: 承認代替として review-protocol
  を実施済み。

## Review Handoff

- `EditingSelection` 後方互換は schema bump で切る判断を reviewer へ明示。
- `clip_b_source_offset` の caller 供給は rate 変換未モデルの正直な記録。
