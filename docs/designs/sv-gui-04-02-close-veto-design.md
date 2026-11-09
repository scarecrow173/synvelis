---
id: "DESIGN-034UqwNDJrureYsAqmckuw"
type: "design"
status: "approved"
title: "SV-GUI-04 slice2 設計 — close_guard op + 末尾 window veto"
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
  implements:
    - "docs/specs/sv-gui-04-02-close-veto.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-04-slice2-close-veto-live-window-guard-query-app-global-reasons.md"
    - "docs/specs/sv-gui-04-02-close-veto.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-02-close-veto.md"
    - "docs/adr/sv-gui-04-slice2-close-veto-live-window-guard-query-app-global-reasons.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-02-close-veto.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 設計: close_guard op + 末尾 window veto

## Context

`shell.rs` の `WindowLifecycle::CloseRequested` 分岐は
`lifecycle(Closing)` を window へ配送後、無条件
`CloseDecision::Approve` で `respond_close` している
（module doc が "unsaved-guard veto" を設計上の hook と明記）。
project/session/recording の truth は `SynvelisApp` 内の単一
`WorkspaceManager → ProjectSession → ProjectController` 系に
あり、window には帰属しない。`UiRuntime::dispatch_window_intent`
は同期的に `Vec<RoutedEvent>` を返し、`attach_view` が同経路で
`syn.view.attach`/`syn.control.subscribe` を発行している。

## 変更点

### `app.rs`（additive）

- `SynvelisApp::close_veto_reasons(&self) -> Vec<&'static str>`:
  app-global guard の reasons を返す read-only 判定。
  - `unsaved_changes`: `controller().live_revision() !=
    saved().revision`（`saved` が `Some` の場合）、または
    `saved` が `None` で `live_revision().0 > 0`（未保存の
    編集があるが save 基準が無い状態）。
  - `recording_active`: `session().control().recording().is_some()`
    （open recording transaction — Pending/Conflict/Rejected を
    含む未消費 transaction）。
  - `recorder_active`: `self.recorder.active()`（`RecorderSlot`
    stub — RCD-01 系 dispatch が実体を配線するまで常に false、
    読み取りのみ）。
- `recorder` field は private のまま — 判定は app.rs 内に閉じる。

### `service.rs`

- `OPS` に `syn.window.close_guard` を追加
  （`syn.workspace.mount` の後、`syn.view.*` の前）。
- `dispatch` に `"syn.window.close_guard" =>
  Ok(json!({ "reasons": self.app.close_veto_reasons() }))` を追加。
  params は読まない（window を知らない app-global query）。
  Result-op のため `ui_pump`/notify は発生しない。

### `shell.rs`

- module doc の `CloseRequested` 項を decision query 経由の
  記述に更新。
- `CloseRequested` 分岐: `lifecycle(Closing)` 配送は維持し、
  `respond_close` の decision を新設 `close_decision` に委譲。
- `close_decision(window, driver) -> CloseDecision`:
  1. `driver.windows()` が `[window]` のみ（= 末尾 live
     window）かを判定 — topology は shell/driver の知識。
  2. `Intent::new("shell-close-guard-{window}",
     "syn.window.close_guard", {"window_id": ...})` を
     `rt.dispatch_window_intent` で dispatch し、返った全
     `RoutedEvent` を `observe_result` + `post_routed` で
     通常経路に流す（query Result は window の view が未知
     command_id として無視する — `ViewHost::apply_receipt` は
     未知 id を早期 return）。
  3. 自 intent id の `Result` を収集: `ok && reasons が空配列`
     のみ clean。それ以外（op error・Result 欠落・malformed
     payload）は guarded=true に倒す（fail-closed）。
  4. `last_window && guarded` → `Veto`、それ以外 → `Approve`。

### `tests/close_veto.rs`（新規）

`tests/sv113_dock.rs`/`tests/two_window.rs` と同じ
`NativeShell` + `FakeDriver` fixture パターン。dirty 化は
seal 前に `app.workspace_mut().session_mut().apply(
ProjectOp::SetProjectName, …)` で行う（`saved=None` かつ
`live_revision>0` の未保存状態）。

## 検証

- `tests/close_veto.rs`: dirty 末尾 window → Veto（window・
  session・view 生存）、clean → Approve→Destroyed→exit、
  dirty 非最終 window → Approve、quit 中の一部 veto、
  `syn.window.close_guard` の reasons 内容。
- `mise run fmt-check`/`check`/`lint`/`test`/`build` +
  native 系タスク（存在する範囲）。
- 既存 `tests/shell.rs` の close/quit テストは clean boot
  project（`saved=None`, `live_revision=0` → clean）で
  `Approve` 経路を維持するため非回帰。
