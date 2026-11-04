---
id: "PLAN-034AUT01Zx4wQn8vF2sLrT6m"
type: "plan"
status: "approved"
title: "AUT-01 Touch Return・Writer State 境界 実装計画"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/131"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
  implemented-by:
    - "docs/tasks/aut-01-task1-touch-return-writer-state.md"
  depends-on:
    - "docs/plans/pc-c1-recording-transaction.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
  references: []
  defers: []
  deferred-by: []
---

# AUT-01 実装計画

## Scope Split

単一 scope:`synvelis-workspace` の `ControlSession` 状態機械拡張 +
`synvelis-project` の `ReferenceSource::Return` variant 追加 +
acceptance test。`synvelis-player`/`recording`/`gateway`/UI 層は
スコープ外(compile に従う機械的追従のみ)。

## File Map

| File | 変更 |
| --- | --- |
| `crates/synvelis-project/src/control.rs` | `ReferenceSource::Return` variant 追加(評価 breakdown の出所表示) |
| `crates/synvelis-workspace/src/control.rs` | `ControlGestureFixation`・`TouchReturn`・`WriterState`・`WriterCloseReason`・`WriterClosure` 追加。session clock(`clock_now`/`clock_advance`/`set_return_duration`)。`gesture_begin` に fixation(+revision 引数)。`gesture_end`(Touch)→return 開始。`evaluate` の return 補間。`record_start(project, revision)` で Write 基準値 seed。境界イベント閉鎖(stop/seek/wrap/play edge/focus/plugin crash/target deleted)。`note_project_changed(project)`。`InspectorBreakdown.writer_state` |
| `crates/synvelis-workspace/src/session.rs` | `note_project_changed(project)` 呼出更新(apply/undo/redo) |
| `crates/synvelis-workspace/src/controls_ui.rs` | `begin_param`/`record_start` の新 signature 追従 |
| `crates/synvelis-workspace/src/lib.rs` | 新型の export |
| `crates/synvelis-workspace/tests/aut_01.rs` | test-spec Rule 群(state matrix + return + fixation + write seed) |
| `crates/synvelis-workspace/tests/pc_c1.rs`・`pc_a.rs`・`pc_b.rs` | `gesture_begin`/`record_start` 新 signature 追従 |

## Steps

1. `synvelis-project/src/control.rs`: `ReferenceSource::Return` 追加。
2. `synvelis-workspace/src/control.rs`: 新型・clock・return・closure・
   fixation・Write seed を実装。`evaluate`/`lower`/`inspector_breakdown`
   が return-aware になる。`transport_play/stop/seek/advance(wrap)`、
   `note_focus_lost`/`note_plugin_crash`/`note_project_changed` に
   明示閉鎖を配線。`gesture_begin`/`record_start` signature 変更。
3. 呼出側更新:`session.rs`・`controls_ui.rs`・既存 test。
4. `tests/aut_01.rs`: test-spec の Rule 群を実装。
5. `mise run fmt` → `check` → `lint` → `test` → `test-rt` →
   `test-msrv`。`git fetch origin && git merge origin/main` 後に再検証。
6. impl record + Shadow CI classify 証跡(`docs/impl/exp/`)作成。

## Acceptance

- `aut_01.rs` 全 test PASS、既存 `pc_*`/`sv47_controls` test が新 API
  で PASS。
- `mise run` 全 gate PASS・warning なし。
- Issue #131 の Start 範囲(状態機械+時計制御コア)を headless に
  検証 — Plugin binding/GUI 接続なし。
