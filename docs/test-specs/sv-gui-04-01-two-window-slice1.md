---
id: "TSPEC-Vjrs2MxmcQmuKhfJmVUxJe"
type: "test-spec"
status: "approved"
title: "SV-GUI-04 slice1 テスト仕様 — 実 2-window mount"
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
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
    - "docs/tasks/sv-gui-04-01-two-window-slice1.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 テスト仕様

## 範囲

headless lane: `FakeDriver` + `NativeShell` + 2 `ViewHost`
（`mount_main_view`/`mount_secondary_view`）の fixture
（`crates/synvelis-app/tests/two_window.rs`）。native lane は
`run-app-native` の実機試行として別記録する。

## ケース

### 2 window mount と同一 project

- 2 window が `Ready` → `attach_view` で `view-1`/`view-2` に
  attach され、`syn.view.list` が両 view と各 owner window を返す。
- 両 window からの `syn.project.info` が同一 `project_id`/
  `revision` を返す（同一 Project session）。

### window ごとの独立 view state

- view-1 で transport gesture（begin/commit）しても view-2 の
  host binding に receipt/preview が乗らず、逆も同様
  （receipt は `view_id` demux で owner window のみへ translated
  Result として届く）。
- `syn.control.projection` row は `(view, target)` 単位で両 view に
  push され、translated Notify は各 owner window に届く
  （同 Target の一貫した Display）。

### cross-window `origin_view` 拒否

- window B 上で `origin_view: "view-1"`（window A の view）の
  `rvf.ui.command.v1` intent は `invalid_params` で拒否され、
  `syn.control.receipt` は発行されない。
- 逆方向（window A で `origin_view: "view-2"`）も同様に拒否。
- owner window 上の正しい `origin_view`（window B で `view-2`）
  は `applied` receipt まで到達する。

### secondary spec

- `secondary_screen_spec` が transport status Label・Play/Stop
  Button・Seek を含み、`control_id == target` を維持する
  （`transport/position`・`transport`）。
- `mount_secondary_view` が `view-2` の `ViewHost` を返し、
  `origin_view` が `"view-2"` になる。

## Non-Goals（NotRun）

- 実 OS E2E の Dock detach/reattach・close veto・monitor 失踪・
  DPI/IME・Plugin Dock（#176/#388/#389/#114 owner）。
- `DockWorkspace` の detach/reattach の app UX 露出（別 slice）。
- Browser/WebView lane の結果を Native PASS へ昇格しない。
