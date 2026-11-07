---
id: "SPEC-ZXEWFEbx3k4Jeu4YDEQcD0"
type: "spec"
status: "approved"
title: "SV-GPUI-01 Native App Shell — Driver-neutral shell・GPUI Driver組立・typed ViewModel↔Binding adapter・app lifecycle"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/386"
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
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv175-native-shell-inventory.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GPUI-01 Native App Shell 仕様

## What Should Be Built

Issue #175（SV-GPUI-01・2026-11-07 横断レビュー確定仕様 v2）に
基づく、製品 Desktop Native GUI の**初期 slice**。Synvelis が Own
するのは **Native composition root・GPUI Driver 組立・non-RT
typed ViewModel↔Ruviform Binding adapter・app lifecycle** のみ。
#112 実画面・#113 Dock UX・#176 plugin dock・#114 製品 E2E・#177
Theme（統合済み）は別 Issue の成果物を呼び出す形に留め、重複
実装しない。

1. **`synvelis-app` の driver-neutral shell（headless lib）**:
   `WindowDriver` port を消費する `NativeShell`。`UiRuntime` +
   `AppService`（既存 `syn.*` op 面・単一 project 正本）を保持し、
   `DriverEvent` を RF386 契約どおりに routing する:
   - `Windowed { Action(intent) }` → `dispatch_window_intent`、
     戻り `RoutedEvent` は `post_event` で origin window へ返送。
   - `Tick` → `drain_jobs_routed` + 共有 notify outbox の drain
     （ready window へ broadcast、view 側が `view_id` で絞る）
     + UI clock 前進 + quit flag 確認。
   - `Windowed { CloseRequested }` → `lifecycle closing` event を
     その window へ送り close policy を適用（初期 slice は
     `Approve`。unsaved guard は後続）。
   - `Windowed { Ready }` → 初期 app-state notify 送信。
   - `Windowed { Destroyed }` → `cancel_window_jobs` + 当該 window
     の view を `syn.view.close` で teardown（generation bump・
     subscription 解放）+ 最後の window なら `exit()`。
   - `syn.view.attach`/`detach`/`close` intent の result を追跡し、
     view→window 対応を shell 側でも保持。
   - `syn.app.quit`（quit flag）→ 全生存 window へ `request_close`。
2. **非 RT typed ViewModel↔Ruviform Binding adapter（headless
   lib）**: native view（RF387 renderer が駆動する Rust 側 view）
   が使う typed adapter `ViewBinding`。
   - `syn.view.attach`/`detach`/`close`、`syn.control.subscribe`/
     `unsubscribe`、`syn.control.dispatch`（typed `CommandIntent`
     — `begin`/`preview`/`commit`/`cancel`/`action` — と
     `DispatchContext`）、`syn.project.*`、`syn.transport.set`、
     `syn.app.*` の `Intent` を生成して outbox へ積む。view 側は
     `take_outbox` で取り出し driver の `Action` 経路へ渡す。
   - `apply(&Event)` で inbound `Event` を typed notice へ demux
     （result 相関・`syn.control.receipt`/`syn.control.projection`/
     `syn.view.state`/`syn.viz.*` channel 振分・lifecycle・job・
     error）。`view_id` 不一致の control notify は `Ignored`。
   - attach result の `generation` を authoritative epoch として
     採用し、以後の `DispatchContext.generation` に一貫適用する
     （RF#383 `ViewLink.attach` generation seam と同じ契約）。
   - 第二の domain truth を作らない: binding lifecycle/gesture state
     machine/receipt ordering は backend `ControlsUi` が正本のまま。
   - GPUI/OS/DOM 型・`gpui` crate 依存を持たない。RT 禁止。
3. **GPUI Driver 組立（composition root 例外・feature `native`
   gate）**: `ruviform-ui-gpui`（feature `gpui`）の `GpuiDriver`
   を組み立てる `bin synvelis-native`。`WindowSpec` で shell window
   を作成し `driver.run(tick, handler)` で `NativeShell` を駆動。
   `gpui` 型・OS 型はこの bin と ruviform-ui-gpui 内だけに閉じる。
   通常 View/Domain/headless crate へ漏らさない。
4. **後継 ADR**: `docs/adr/sv-gui-01-…` の SolidJS/WebView 採用決定
   を GPUI Driver-neutral Native Desktop 方針で明示 supersede。
   旧実装・旧 frontend・PoC 証拠は Native 実証まで削除しない。

## Acceptance Criteria

- `mise run fmt-check`/`check`/`lint`/`test` が green で、headless
  依存境界（`mise run check-headless-deps`）を維持する。headless
  crate に `gpui`/`ruviform-ui-gpui` が混入しない。
- `NativeShell` が `FakeDriver` で headless 検証可能:
  window 作成→Ready→初期 notify、Action intent→typed result 返送、
  `syn.view.attach`→`syn.control.dispatch`(gesture begin→preview→
  commit/cancel)→receipt/projection notify 到達、`syn.project.new`/
  save→store 書込・再 open、CloseRequested→closing event+Approve→
  Destroyed→view close teardown+最終 window で exit、quit flag→
  request_close。
- `ViewBinding` が typed intent を正しい wire 形（op/params/
  `DispatchContext.generation` は attach 応答の epoch）で生成し、
  result 相関・notify channel demux・view_id 絞込を行うことを
  headless test で検証する。
- composition root/隔離 bin 以外の通常コードが `gpui`・OS window
  型を参照しない（依存検査で固定）。
- `synvelis-app` の `native` feature lane（`mise run check-app-native`
  /`lint-app-native`）が通るか、環境制約で未検証なら NotRun と
  理由を明記する。実 OS window・GPU・IME・focus は観測しない限り
  Pass としない。
- 最小 Tab+Fader slice は RF387 実 API 未入手の間は NotRun と明記
  し、必要契約を discovery doc に固定する。
- 後継 ADR が追加され旧 ADR に `superseded`/`superseded-by` が付く。

## Explicitly Out of Scope

- RF387 Component/ViewSpec renderer（別 worker・候補待ち）、実画面
  （#112）、Dock UX（#113）、実 plugin dock（#176）、Theme 再実装
  （#177 済）、製品 E2E（#114）、実 desktop input/IME（RF389 owner）、
  旧 WebView/frontend の撤去、新外部依存の追加。
