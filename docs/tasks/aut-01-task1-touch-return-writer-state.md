---
id: "TASK-034AUT01Mn7pQs4wXc9vBhJk"
type: "task"
status: "in-progress"
title: "AUT-01: Touch Return 150ms smoothstep・Read/Touch/Latch/Write 境界 State 実装"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/131"
  changes:
    added:
      - "crates/synvelis-workspace/tests/aut_01.rs"
      - "docs/impl/exp/issue-131-aut-01-shadow-ci.md"
    modified:
      - "crates/synvelis-project/src/control.rs"
      - "crates/synvelis-workspace/src/control.rs"
      - "crates/synvelis-workspace/src/session.rs"
      - "crates/synvelis-workspace/src/controls_ui.rs"
      - "crates/synvelis-workspace/src/lib.rs"
      - "crates/synvelis-workspace/tests/pc_a.rs"
      - "crates/synvelis-workspace/tests/pc_b.rs"
      - "crates/synvelis-workspace/tests/pc_c1.rs"
      - "crates/synvelis-workspace/tests/sv47_controls.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/aut-01-touch-return-writer-state.md"
  implemented-by:
    - "docs/impl/ir/aut-01-touch-return-writer-state.md"
  depends-on:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
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
    - "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
  references: []
  defers: []
  deferred-by: []
---

# AUT-01 Task 1: Touch Return・境界 State・fixation・Write seed・検証

## Goal

Issue #131 の Start 範囲(状態機械+時計制御コア)を `synvelis-workspace`
の `ControlSession` に実装し、test-spec の Rule 群を検証する。
Plugin binding/GUI 接続は行わない。

## Steps

1. `synvelis-project/src/control.rs`: `ReferenceSource::Return` variant。
2. `synvelis-workspace/src/control.rs`:
   - `ControlGestureFixation`・`TouchReturn`・`WriterState`・
     `WriterCloseReason`・`WriterClosure`・`WRITER_CLOSE_CAP`。
   - `clock_now`/`clock_advance`/`set_return_duration`/`return_duration`。
   - `gesture_begin(project, target, at, revision)` で fixation 固定 +
     active return 除去。
   - `gesture_end`(Touch)→ `TouchReturn` 開始(release 値 = 最終
     override)。`evaluate` で manual>latch>return の順に補間、
     `reference_source` を `Return` に訂正。
   - `record_start(project, revision)` — Arm+write-mode 固定集合の
     Write target へ reference 基準値を `manual_overrides` に seed。
   - 境界閉鎖:`transport_play`(new-play edge)/`transport_stop`/
     `transport_seek`/`transport_advance` wrap/`note_focus_lost`/
     `note_plugin_crash`/`note_project_changed(project)`(消えた
     target 閉鎖)。`set_mode` の latch/write-held 除去を記録。
   - `writer_state`/`writer_closes`/`gesture_fixation` introspection。
     `InspectorBreakdown.writer_state`。
3. `session.rs`/`controls_ui.rs`/既存 test の call site 更新。
4. `tests/aut_01.rs`: test-spec Rule 群。
5. `mise run fmt` → `check` → `lint` → `test` → `test-rt` →
   `test-msrv`。`git fetch origin && git merge origin/main` 後に再検証。
   impl record・exp 証跡・PR。

## Acceptance

- `aut_01.rs` 全 test PASS、既存 `pc_*`/`sv47_controls` test が
  新 API で PASS。
- `mise run` 全 gate PASS・warning なし。
