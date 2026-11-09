---
id: "SPEC-034UogGZ0QtZaMjO9XxPXv"
type: "spec"
status: "approved"
title: "SV-GUI-04 slice1 precursor: multi-window WindowHandle + headless dock fixture"
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
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
    - "docs/plans/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/tasks/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md"
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

# SV-GUI-04 slice1 precursor 仕様: multi-window WindowHandle + headless dock fixture

## What Should Be Built

Issue #113（SV-GUI-04 Multiwindow Workspace/Dock）の **precursor
スライス**。headless 契約層のみを対象とし、実 GPUI window 生成・
2-window mount・`synvelis-native` 配線は後続スライスに送る
（app 層は別 dispatch が並行編集中のため本スライスでは触れない）。

1. `crates/synvelis-app/src/host.rs` に multi-window 対応の
   `WindowHandle` 実装 `HeadlessWindowHost` を追加する。
   `ShellWindowHost::create_window` の `BackendUnavailable` は
   **置き換えない** — 製品 shell の単一 OS window 契約は維持し、
   headless/fake 経路で複数 `WindowId` identity を追跡できる
   契約を別型として提供する。
2. `crates/synvelis-app/tests/sv113_dock.rs`（新規）で fixture
   検証を行う:
   - 2 window identity が同一 project を参照できる
     （Project/Job が window 寿命と独立）。
   - window ごとの独立 view state。
   - cross-window `origin_view` の拒否（`shell.rs` 既存の
     validation 経路を `NativeShell` + `FakeDriver` で駆動）。
   - `DockWorkspace` 経由の `begin_transfer`/`commit`/`abort`
     が app host 契約から到達できる。

## Why Now

Issue #113 の受入条件は実 2Window の OS 検証を要求するが、app 層
（`screens.rs`/`shell.rs`/`ui_ops.rs`/`app.rs`/`service.rs`/`bin/`）
は別 dispatch が slice 2 として編集中であり、本スライスで触ると
競合する。先に headless 契約層 — window identity の発行・追跡・
close/destroy semantics と dock transfer の到達性 — を確定させる
ことで、後続の app 層スライスは検証済みの契約に対して実装できる。

## Users and Value

- Synvelis 開発者 / 後続スライスの Worker: multi-window の
  headless 検証契約が先に存在するため、app 層実装時に
  「window identity・view state 独立性・origin 検証・transfer
  rollback」の正しさを OS 無しで回帰できる。
- Issue #113 の受入証拠: headless lane の Pass を実 OS/Native
  PASS と区別して記録するための土台。

## Scope

### In Scope

- `crates/synvelis-app/src/host.rs`: `HeadlessWindowHost` 追加
  （`WindowHandle` 実装、複数 `WindowId` の発行・記録・
  live 集合管理、`fail_create` 注入）。
- `crates/synvelis-app/src/lib.rs`: 新規型の pub re-export
  （additive のみ）。
- `crates/synvelis-app/tests/sv113_dock.rs`: 新規 fixture test。
- `docs/` 文書チェーン（本 spec → design → plan → task →
  test-spec）。

### Out of Scope

- `ShellWindowHost::create_window` の挙動変更（`BackendUnavailable`
  を維持）。
- 実 GPUI window 生成、`create_view_window` の実 backend 接続、
  `synvelis-native` での 2 窓 mount、`screens.rs` への 2nd screen
  spec、Detach→Reattach の実 GUI 動作、close-cancel、monitor
  失踪、DPI/IME — 全て NotRun（後続スライス）。
- `synvelis-workspace`/`synvelis-project`/`synvelis-player` 等他
  crate、`Cargo.toml`/`Cargo.lock` の変更。
- `screens.rs`・`shell.rs`・`ui_ops.rs`・`app.rs`・`service.rs`・
  `bin/` の変更（別 dispatch の編集範囲）。

## Requirements

- `HeadlessWindowHost` は `ruviform_ui::window::WindowHandle` を
  実装し、`create_window` が単調増加・非再利用の `WindowId` を
  発行する（`WindowId` 契約: nonzero・session 内一意・destroy 後
  再利用なし）。
- fallible 操作（`resize`/`focus`/`respond_close`/`destroy_window`）
  は未知 window に `UiError::Host` を返し、fire-and-forget 操作
  （`eval`/`request_close`）は未知 window を無視する —
  `WindowHandle` の delivery safety contract に従う。
- `respond_close(Approve)` は window を live 集合から除去し
  `Destroyed` 相当の記録を残す。`Veto`/`Defer` は window を維持する。
- 全 host 呼出しは観測可能な記録（created configs・evals・
  resizes・focuses・close requests/decisions・destroys・exited）
  として残し、test が契約を assert できるようにする。
- `WorkspaceManager<HeadlessWindowHost>` 経由で
  `create_window`/`spawn_view`/`begin_transfer`/`commit_transfer`/
  `abort_transfer`/`detach_to_new_window`/`reattach`/
  `on_window_event` が headless に完結する。
- `NativeShell` + `ruviform_ui_test::FakeDriver` 経路で、別 window
  に attach された view を `origin_view` に指す view command が
  `invalid_params` で拒否されることを検証する（`shell.rs` 既存
  validation、変更なし）。

## Acceptance Criteria

- [x] `HeadlessWindowHost` が `WindowHandle` を実装し、2 つの
  `WindowId` が同一 `ProjectSession`/`Project` を参照できる
  fixture test が Pass する。
- [x] window destroy（`DetachViews` disposition）後も Project・
  view registry・view state が保持され、view が detached pool へ
  移り `reattach` で復帰できる fixture test が Pass する。
- [x] window ごとに独立した view state（tool・viewport）と
  focus→active view 解決を持つ fixture test が Pass する。
- [x] cross-window `origin_view` が `invalid_params` で拒否され、
  同一 window の `origin_view` は受理される shell 経路 test が
  Pass する。
- [x] `begin_transfer`→`commit_transfer` で view ownership が
  window 間を移動し、`begin_transfer`→`abort_transfer` で source
  ownership が維持される fixture test が Pass する。
- [x] `ShellWindowHost::create_window` が引き続き
  `BackendUnavailable` を返すことを pin する test が Pass する。
- [x] `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` が全て Pass する。
- [ ] 実 2Window 起動・`synvelis-native` 2 窓 mount・実 OS
  Desktop/Dock/Native Plugin Gate — **NotRun**（後続スライス。
  headless lane の結果を Native PASS に昇格しない）。

## Deferred Design Concerns

- 実 GPUI `WindowDriver` backend での 2 窓 mount と
  `create_view_window` 接続 | app 層が別 dispatch で編集中・
  実 OS 検証は #114 の Gate | app 層スライス完了後 | headless
  契約と実 backend の semantics が乖離したまま残る。
- `WindowCloseDisposition::RelocateTo`・close veto・monitor 失踪・
  layout restore の headless 検証拡充 | 本スライスは detach/abort/
  共有 project の核契約に限定 | 後続スライス | preset/restore
  経路の回帰が後手になる。

## Sources and Relations

- Issue #113（SV-GUI-04）受入条件のうち本スライスが担う範囲:
  「2 Window から同一 Project を参照」「Window close で
  Project/Job が暗黙停止しない」「Fake/Headless と実 OS Gate を
  別 Capability として記録」。
- 上流: WS-A（`docs/specs/ws-a-workspace-foundation-*.md` —
  `WorkspaceManager`/`DockWorkspace`/`WindowHandle` 契約）、
  SV-GPUI-01（`docs/specs/sv-gpui-01-native-app-shell.md` —
  `NativeShell`/`WindowDriver`/`FakeDriver` 経路）。
- Ruviform 側の window/dock 契約は ruviform #181/#182/#386
  （`WindowHandle`・`WindowDriver`・`DockWorkspace`）に依存し、
  本スライスでは変更しない。
