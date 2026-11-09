---
id: "DESIGN-Ywl9hP3wdlHWjoMPB7Hvjn"
type: "design"
status: "approved"
title: "SV-GUI-04 slice1 設計 — 2nd view spec + env-gated 2窓 mount + FakeDriver fixture"
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
  implements:
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-04-01-two-window-second-view.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/adr/sv-gui-04-01-two-window-second-view.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 設計: 実 2-window mount

## Context

`NativeShell`（shell.rs）は `windows: BTreeMap<WindowId, bool>`・
`views: HashMap<String, WindowId>`・`view_generation`・
`attach_view`・`on_view_command` の `origin_view` owner 検証・
`Destroyed` teardown を既に実装済み。`synvelis-native` は
`GpuiDriver::create_view_window` + `ViewMount{host, theme}` で
main view を mount し、`Ready` で `attach_view`、state 差分を
`update_view` で push する。`syn.view.attach` は任意の
`view_id`/`window_id` を受理し、`ViewHost` は
`origin_view = "view-{ViewInstanceId}"` を mint する。

## 変更点

### `screens.rs`（additive）

- `SECONDARY_VIEW_INSTANCE_RAW: u64 = 2`、
  `secondary_view_id() -> "view-2"`、
  `secondary_view_instance() -> ViewInstanceId(2)`。
- `secondary_view_targets()` — `[TRANSPORT_TARGET,
  TRANSPORT_POSITION_TARGET]`（transport deck が駆動する target のみ）。
- `secondary_screen_spec(&MainScreenState) -> ViewSpec` —
  `Panel("screen2.root") > [transport_status_label,
  transport buttons panel, transport_seek]`。既存の private
  builder（`transport_status_label`/`transport_button`/
  `transport_seek`）を再利用し、`control_id == target` 契約を維持。
- `mount_secondary_view(attach_epochs, state)` —
  `mount_main_view` と同じ allocator mint 経路。共通部は private
  helper に括る。

### `lib.rs`

`secondary_view_id`/`secondary_view_instance`/
`secondary_view_targets`/`secondary_screen_spec`/
`mount_secondary_view` を export に追加（additive のみ）。

### `bin/synvelis-native.rs`

- `SYNVELIS_SECOND_WINDOW` env flag（`1`/`true`/`yes`/`on`
  大小文字非依存で有効、未設定・その他は無効）を process
  boundary で読み、doc comment に記録。
- 有効時: `mount_secondary_view` で第 2 `ViewHost` を mint し、
  `driver.create_view_window`（title `"Synvelis — Secondary"`、
  小さめの `logical_size`/`position` offset、`theme.clone()`）で
  window を予約。
- run loop を mount 記述の list 化: 各 `{window, view_id,
  targets, secondary, attached}` について `Ready` で一度だけ
  `attach_view`、state 差分時に `attached` な各 window へ
  `update_view(window, main_screen_spec|secondary_screen_spec)`。
  `shell.handle` → attach の順序は既存と同じ（window 登録後に
  owner 解決）。

### `tests/two_window.rs`（新規）

FakeDriver + `NativeShell` + 2 `ViewHost`（`mount_main_view` +
`mount_secondary_view`）の fixture。view_mount.rs の
`deliver_to_host`/`pump_view_intents` パターンを 2 host 分複製。

## 検証

- headless: `two_window.rs`（同一 project・独立 view state・
  cross-window `origin_view` 拒否・同 target の receipt/projection
  が各 window の view に届く）。
- native: `check-app-native`/`lint-app-native`/`build-app-native`
  + `SYNVELIS_SECOND_WINDOW=1` での `run-app-native` 実機試行
  （結果を正直に記録）。
