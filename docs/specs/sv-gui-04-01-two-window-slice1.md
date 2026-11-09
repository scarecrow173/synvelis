---
id: "SPEC-OGVSCKWOs5Dv4e9e0K7wDY"
type: "spec"
status: "approved"
title: "SV-GUI-04 slice1: 実 2-window mount（fake driver fixture + env-gated native）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-04-01-two-window-second-view.md"
    - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
    - "docs/plans/sv-gui-04-01-two-window-slice1.md"
    - "docs/tasks/sv-gui-04-01-two-window-slice1.md"
    - "docs/test-specs/sv-gui-04-01-two-window-slice1.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 仕様: 実 2-window mount

## What Should Be Built

Issue #113（SV-GUI-04 Multiwindow/Dock 実 GUI 統合）の第 1
スライス。既存の `ViewSpec → ViewHost → update_view` push 経路・
`MainScreenState` 共有 cell・`NativeShell` の `windows` map +
per-window `attach_view`（PR #199/#201 で main に存在）を使い、
**2 つ目の window に実際の view を mount する最小経路**を実装する。

1. **2nd screen spec**: `screens.rs` に第 2 の `ViewSpec`/view_id
   （`view-2`、`ViewInstanceId(2)`）を追加する。region 構成は
   `main_screen_spec` と別（transport readout + Play/Stop + Seek の
   transport deck）で、同一 `MainScreenState` cell・同一 Project
   session を参照する。
2. **env-gated native mount**: `synvelis-native` が
   `SYNVELIS_SECOND_WINDOW` 設定時に `create_view_window` で 2 つ目の
   window を生成し `attach_view` で接続する。未設定時は従来の
   1 窓挙動を維持する。
3. **fake/headless fixture**: `tests/` に FakeDriver 上の 2-window
   fixture を追加し、2 window が同一 project を参照・window ごとに
   独立した view state を持ち・cross-window `origin_view` が
   shell の既存 validation で拒否されることを検証する。

## Why

#113 の Acceptance「実2Windowを起動し、別Focus・同一Project・
独立ViewState、同Targetの一貫したReceipt/Displayを確認」の最小
実測可能部分。Detach/Reattach・close veto・monitor 失踪などの
Dock UX は後続 slice と #176/#114 の owner 範囲。

## Scope

- `crates/synvelis-app/src/screens.rs`（additive: secondary spec/
  id/targets/mount）、`app.rs`、`ui_ops.rs`、`binding.rs`、
  `bin/synvelis-native.rs`、`lib.rs`（export 追加のみ）、
  `crates/synvelis-app/tests/`、`docs/` チェーン。
- `host.rs` の multi-window WindowHandle 抽象には依存しない
  （別 dispatch が precursor として編集中）。

## Out of Scope

- Detach→Reattach・move abort/rollback の実 GUI 動作、close-cancel、
  monitor 失踪、DPI/IME、Plugin Dock（#176/#388/#389 の owner）。
- `DockWorkspace` の detach/reattach を app UX へ露出（別 slice）。
- `service.rs`・他 crate・`Cargo.toml`/`Cargo.lock`・ruviform 側の変更。

## Acceptance Criteria

- [ ] env-gated native 起動（`SYNVELIS_SECOND_WINDOW`）で 2 窓
  mount を試行し、結果を正直に記録する（成功なら実測 pass、
  不可なら理由つき Partial/NotRun）。fake/headless fixture は必須。
- [ ] headless lane（FakeDriver/ViewHost）と native lane（GPUI
  実 OS Window）を別 Capability として記録し、旧 Browser Pass を
  現行 Native PASS へ昇格しない。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `build` と
  `check-app-native` / `lint-app-native` / `build-app-native` が
  全て exit 0。
