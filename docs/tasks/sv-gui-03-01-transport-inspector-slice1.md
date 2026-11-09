---
id: "TASK-034Ul52jO5RARmW21UHLGR"
type: "task"
status: "in-progress"
title: "SV-GUI-03 slice1: Transport 操作 + Inspector 表示を既存 GPUI 経路に接続"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-03-01-transport-inspector-slice1.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-03-01-transport-inspector-slice1.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/designs/sv-gui-03-01-transport-inspector-slice1-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-01-transport-inspector-slice1.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice1: Transport 操作 + Inspector 表示を既存 GPUI 経路に接続

## Tasks

- [x] `screens.rs`: `MainScreenState` 系 4 型、`main_screen_spec(&MainScreenState)`、
      `main_view_targets()` + `TRANSPORT_TARGET`、Transport status Label +
      Play/Stop Button、Inspector tab（target Label + breakdown Table）、
      `mount_main_view` state 引数化
- [x] `app.rs`: `main_screen: Rc<RefCell<MainScreenState>>` cell、
      `main_screen_cell()`、`publish_project_length()` を cell 更新に変更
- [x] `ui_ops.rs`: `capture_screen_state()` を `ui_pump` 末尾に追加
      （`transport_row`/`strip_row` の observed → state cell）
- [x] `lib.rs`: 新規型 re-export
- [x] `bin/synvelis-native.rs`: `main_screen_cell` + state `PartialEq`
      push の `update_view`
- [x] `tests/view_mount.rs`: fixture screen cell 保持、`push_screen_state()`、
      transport Button/Label/Table/spec-update 検証、既存呼出の state 化
- [x] scenario test: `FakeGuiTest`/`SemanticGuiTest` で gesture→command→
      receipt→projection（`tests/screen_scenario.rs`）
- [x] mise fmt-check/check/lint/test/build + check-app-native/
      lint-app-native/build-app-native
- [x] `run-app-native` 実機確認 + 証拠記録（Play→Playing / Stop→Stopped /
      Inspector tab breakdown 表示を実窓 + ログ + OCR で確認）
- [ ] review-protocol → merge origin/main → 最終検証 → commit/push/PR

## Acceptance（Issue #112 slice 1）

- [x] Transport region: `transport` target に Play/Stop `Button` mount、
      既存 command 経路で操作可能、`TransportProjection` 購読で status
      readout、seek 秒表示規約維持
- [x] Inspector region: `InspectorBreakdown` を `Table` で表示、
      既存 `update_view` push で spec 更新、新規 domain op 無し
- [x] `view_mount.rs` 拡張 + `SemanticGuiTest`/`FakeGuiTest` scenario で
      gesture→command→receipt→projection を headless 検証
- [x] 実 GPUI window（`run-app-native`）で起動・操作確認、証拠を PR 記録
- [x] 文書チェーン（spec→design→plan→task→test-spec）作成

## Notes

- `service.rs` は編集範囲外のため `publish_project_length` の呼出名を
  維持し、本体を `MainScreenState.project_length_seconds` 更新に変更。
  transport/inspector capture は `ui_pump`（ui_ops.rs）側で行う。
- Inspector 対象は購読 param target（現行 mixer gain fader bootstrap）
  のみ。選択 UI は後続 slice。
