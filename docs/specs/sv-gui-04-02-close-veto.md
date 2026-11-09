---
id: "SPEC-034Uqtwgi61id6YJP9tZZX"
type: "spec"
status: "approved"
title: "SV-GUI-04 slice2: close-veto/cancel（unsaved-guard veto）"
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
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-04-slice2-close-veto-live-window-guard-query-app-global-reasons.md"
    - "docs/designs/sv-gui-04-02-close-veto-design.md"
    - "docs/plans/sv-gui-04-02-close-veto.md"
    - "docs/tasks/sv-gui-04-02-close-veto.md"
    - "docs/test-specs/sv-gui-04-02-close-veto.md"
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

# SV-GUI-04 slice2 仕様: close-veto/cancel（unsaved-guard veto）

## What Should Be Built

Issue #113（SV-GUI-04 Multiwindow/Dock 実 GUI 統合）の close-veto
スライス。`NativeShell::on_windowed` の `CloseRequested` が
無条件 `Approve` していた部分を、decision query
（`syn.window.close_guard`）経由の vetoable フローに置き換える。

1. **decision query op**: `syn.window.close_guard` を `syn.*`
   op surface に追加し、アプリ側の guard 状態（未保存変更・
   録音中）を reasons として返す。`attach_view` と同じ
   `rt.dispatch_window_intent` パターンで shell が同期問い合わせ
   する。
2. **veto ポリシー**: 末尾の live window の close のみが
   guard 対象 — 未保存変更（`live_revision` vs `saved().revision`）
   または録音 active（open recording transaction /
   `RecorderSlot::active()`）の間は `Veto`、それ以外は `Approve`。
   decision query 自体の失敗・malformed 応答は安全側（guarded
   扱い）に倒す。
3. **quit フロー**: `syn.app.quit` → 全 live window へ
   `request_close` の既存経路はそのまま。各 window の
   `CloseRequested` が個別に decision query を通るため、
   veto された window は生存し、session/project/graph は
   暗黙停止しない。

## Why

Issue 受入「Detach→Reattach、move abort/rollback、close-cancel、
Window生成失敗・IPC timeout、layout corruption/monitor不足で
安全な復帰」「Dock/Window closeでProject/Graph/Recording/Jobが
暗黙停止せず、誤Command・subscription/GPU漏れなし」の
close-cancel 部分。現行の無条件 `Approve` は dirty project や
録音中でも window close → app exit で session を暗黙破棄し得る
（shell.rs module doc が "unsaved-guard veto" を設計上の hook
として明記済み）。

## Users and Value

- Synvelis デスクトップ利用者: 未保存編集・録音中に window を
  誤って閉じても、最後の window では veto により session が
  保持される。非最終 window の close は session が残存 window に
  生き続けるため通常通り許可される。
- 開発者: close 経路が session 側を巻き込まないことを
  FakeDriver fixture で検証可能になる。

## Scope

### In Scope

- `crates/synvelis-app/src/shell.rs`（`CloseRequested` の
  無条件 `Approve` → decision query 経由の vetoable decision）、
  `crates/synvelis-app/src/service.rs`（`syn.window.close_guard`
  の登録/ディスパッチ）、`crates/synvelis-app/src/app.rs`
  （guard reasons の算出）、
  `crates/synvelis-app/tests/close_veto.rs`（新規）、
  `docs/` チェーン。
- veto 条件: 未保存変更または録音 active → 末尾 window のみ Veto。
- decision query 失敗時の fail-closed（Veto 側）設計。

### Out of Scope

- Detach→Reattach / dock transfer の GUI op 配線（別 slice、
  既知ブロッカーあり）。
- `synvelis-workspace/`・`synvelis-recording/`・
  `synvelis-project/` の変更（別 dispatch 稼働中。
  `recorder.active()` の読み取りのみ）。
- Ruviform 側 Window/Host/GPUI Driver の変更。
- `tests/two_window.rs`・`tests/sv113_dock.rs` の変更。
- 「保存しますか?」プロンプト等の非同期確認 UX（`Defer`
  decision の活用は後続 slice）。
- monitor 失踪時 layout restore（#177）、DPI/IME（#388/#389/#114）、
  Plugin Dock（#176）。
- 実 OS（Windows/macOS/Linux）での Pass/Fail/veto/focus 証拠
  （#114 gate、NotRun 記録）。IPC timeout（GPUI native 経路
  NotRun）。

## Requirements

- `CloseRequested` 受信時、shell は `lifecycle(Closing)` を当該
  window へ配送した上で `syn.window.close_guard` を
  `dispatch_window_intent` で同期問い合わせし、結果を
  `CloseDecision` に写像して `respond_close` する。
- guard reasons は少なくとも `unsaved_changes`
  （`live_revision != saved.revision`、`saved` 無しで
  `live_revision > 0` の場合を含む）と録音 active
  （open recording transaction または `RecorderSlot::active()`）
  を区別して報告する。
- veto は末尾 live window の close にのみ適用する。decision
  query の失敗・Result 欠落・malformed payload は guarded
  （fail-closed）として扱う。
- veto された window は `Destroyed` されず、view/subscription/
  project/session state を保持する。close が approve された
  window は従来通り `Destroyed` teardown（`syn.view.close`
  replay）を行う。

## Acceptance Criteria

- [ ] close-cancel で安全な復帰: dirty 状態の末尾 window への
  `CloseRequested` が `Veto` となり、window・session・project
  が保持される（FakeDriver fixture で検証）。
- [ ] clean 状態の close は `Approve` → `Destroyed` teardown →
  末尾 window なら `exit()`（既存契約の維持）。
- [ ] quit 中の一部 veto: 複数 window + dirty で `syn.app.quit`
  すると、末尾として処理された window のみ Veto で生存し、
  session/project は暗黙停止しない。
- [ ] Dock/Window close で Project/Graph/Recording/Job が暗黙
  停止せず、誤 Command・subscription/GPU 漏れなし（close 経路で
  session 側が巻き込まれないことを fixture で検証）。
- [ ] decision query 自体の失敗・malformed は安全側
  （fail-closed → Veto）に倒す設計を含める。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `build` と
  `check-app-native` / `lint-app-native` / `build-app-native`
  が全て exit 0（存在する範囲で）。

## Deferred Design Concerns

- 「保存しますか?」確認ダイアログ / `CloseDecision::Defer` を
  使った非同期 close 継続 — reason: 本 slice は veto/cancel の
  最小経路。re-engagement: 保存 prompt UX の slice 起票時。
  risk if ignored: veto は「閉じない」までで、破棄→close の
  明示的経路は `syn.project.reload`/`new`/`open` が既に存在。
- per-window の guard（window 固有の未保存 view state 等）—
  reason: project/session/recording は app-global で window に
  帰属しない。re-engagement: window 固有 state が永続化対象に
  なった時点。

## Sources and Relations

- Issue #113 受入条件（close-cancel・Window close while
  Recording/Render → Domain Truth 不変）。
- `docs/specs/sv-gui-04-01-two-window-slice1.md`（2-window mount
  基盤）、`docs/specs/sv-gui-04-01-headless-window-handle-dock.md`
  （headless WindowHandle/dock fixture）。
- `shell.rs` module doc の "unsaved-guard veto" 設計 hook 記述、
  `ruviform-ui` `CloseDecision`/`WindowDriver::respond_close`
  契約（RF386）。
