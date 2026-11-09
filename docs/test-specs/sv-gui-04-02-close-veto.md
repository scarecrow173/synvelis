---
id: "TSPEC-034UqwToZSbEbuOAgyC2SB"
type: "test-spec"
status: "approved"
title: "SV-GUI-04 slice2 テスト仕様 — close-veto/cancel"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-02-close-veto.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-04-02-close-veto.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 テスト仕様 — close-veto/cancel

## Purpose

`CloseRequested` → `syn.window.close_guard` → `CloseDecision`
写像の close policy 契約を固定する。window 破壊の可否という
取り返しのつかない判断を、FakeDriver + NativeShell の
決定的 fixture で headless 検証する。

## Feature

```
Feature: Window close guard (unsaved/recording veto)
  As a Synvelis user
  I want the last window's close request to be vetoable
  So that unsaved project changes and active recording are never silently discarded
```

## Rules

- RULE-1: 末尾 live window の `CloseRequested` は
  `syn.window.close_guard` の結果を `CloseDecision` に写像する。
- RULE-2: guard reasons が空なら `Approve`、非空なら `Veto`。
- RULE-3: guard query の失敗・Result 欠落・malformed payload は
  guarded 扱いとし、末尾 window では `Veto` に倒す（fail-closed）。
- RULE-4: 非最終 window の close は guard 状態に関わらず
  `Approve`（残存 window が session を保持する）。
- RULE-5: veto は `Destroyed` を発生させず、window・view・
  subscription・project/session/job 状態を保持する。
- RULE-6: `Approve` は従来通り `Destroyed` teardown →
  view close replay → 末尾で `driver.exit()`。
- RULE-7: `syn.app.quit` の per-window `request_close` は各 window
  で同じ decision 経路を通り、一部 veto が起き得る。
- RULE-8: `syn.window.close_guard` の Result は
  `reasons` 配列を含み、`unsaved_changes` / `recording_active` /
  `recorder_active` を報告する（recorder は stub で false）。

## Examples

- EX-1: clean boot project（saved=None, live_revision=0）の
  単一 window に `CloseRequested` → `Approve` → `Destroyed`
  → `exit()`。
- EX-2: dirty project（`saved=None`, `live_revision>0`）の
  単一 window に `CloseRequested` → `Veto`、window・session・
  view・subscription 生存、`exited=false`。
- EX-3: clean project の 2 window で片方を close → 非最終
  window は dirty 判定に関わらず `Approve` → `Destroyed`、
  session/project は残存 window で無傷。
- EX-4: dirty project で `syn.app.quit` → 全 live window に
  `request_close` が発行され、処理順で先に approve された
  window は destroy、最後に処理された window は `Veto` で
  生存（quit の一部 veto）。project/session は暗黙停止しない。
- EX-5: `syn.window.close_guard` op を直接呼ぶと
  `{"reasons": [...]}` が返り、dirty 状態で `unsaved_changes`
  を含む。
- EX-6: decision query の失敗/malformed（op 未登録経路を
  模擬する等、実装可能な範囲で）→ 末尾 window は `Veto`。
  実装上不可能な場合は design の fail-closed 経路をコード
  レビューで確認し、本例は NotApplicable と記録する。

## Verification

- `cargo test -p synvelis-app --test close_veto`（mise run test
  の一部として実行）。
- 既存 `tests/shell.rs` の close/quit テスト（clean boot →
  Approve 経路）が非回帰であることを `mise run test` で確認。
- 実 OS での close 証拠は #114 gate のため本スライスでは
  NotRun。
