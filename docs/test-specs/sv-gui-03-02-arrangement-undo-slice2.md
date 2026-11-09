---
id: "TSPEC-t3FjoDxDu6K8hfJWqp6Fz1"
type: "test-spec"
status: "approved"
title: "SV-GUI-03 slice2 テスト仕様 — Arrangement read-only Table + Undo/Redo"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/designs/sv-gui-03-02-arrangement-undo-slice2-design.md"
    - "docs/tasks/sv-gui-03-02-arrangement-undo-slice2.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice2 テスト仕様

## 範囲

headless（`FakeDriver`/`ViewHost`/`NativeShell`/`AppService`）と
workspace 単体。実機 lane は `run-app-native` の目視確認。

## ケース

### workspace `arrangement_rows` 単体（`crates/synvelis-workspace/tests/`）

- AddTrack/AddLane/AddClip 後、`arrangement_rows` が track 名・
  lane/clip/source tagged id・start/end 秒・muted を返す。
- track→lane→clip の順序（`lane_ids`/`clip_ids` の永続化順）で
  ソートされる。
- clip span 未解決（source 無し等）は start/end = `None` で行が残る。

### `view_mount.rs`

- mount 直後の spec に Arrangement tab・Table（7 columns・
  `selection: None`）・Undo/Redo button が含まれる。
- fixture で `session.apply(AddTrack/AddLane/AddClip)` 後、
  `app.publish_project_length()`（または `main_screen_cell`）で
  `MainScreenState.arrangement_rows` が更新され、`update_view`
  push で driver が新 spec を受け取る。
- Undo/Redo button 押下 → `dispatch_project_action` →
  `syn.project.undo`/`redo` op が dispatch され結果 event が
  post される（履歴空なら `undone:false`/`redone:false`）。

### `ui_ops.rs`

- `op_project_undo`/`op_project_redo` が `ControlsUi::undo`/`redo`
  を呼び、履歴空で `{"undone": false}`/`{"redone": false}` を返す。
- project 変更後は `true` を返し、`ui_pump` で購読 binding の
  receipt/projection が流れる。

## Non-Goals（NotRun）

- 実 OS E2E、pixel golden、IME、Dock/2window、Meter/Waveform、
  row selection→Inspector 接続。
