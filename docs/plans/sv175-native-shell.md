---
id: "PLAN-iD5fujcWIovHuywc0CIEeu"
type: "plan"
status: "approved"
title: "SV175 Native App Shell 実施計画"
created: "2026-11-07"
updated: "2026-11-07"
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
    - "docs/designs/sv175-native-shell-design.md"
  implemented-by: []
  depends-on:
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/designs/sv175-native-shell-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv175-01-native-shell-binding.md"
    - "docs/test-specs/sv175-01-native-shell-binding.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv175-native-shell-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv175-01-native-shell-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# SV175 Native App Shell 実施計画

## 工程

1. ruviform git dep を main `e8adcc09` 系へ更新（`Cargo.lock` 再生
   成・`sync-ui-deps` で vendor REV 整合）。
2. `SynvelisApp` に `ui_clock()` 共有 cell accessor を追加し、
   `set_ui_now`/`ui_pump` を同一 cell 経由に内部統一（API 非破壊）。
3. `src/binding.rs` `ViewBinding`（typed intent 生成 + Event
   demux）を実装。
4. `src/shell.rs` `NativeShell`（driver-neutral routing/lifecycle/
   teardown/quit）を実装。
5. `native` feature（optional `ruviform-ui-gpui`+`gpui`）と
   `bin synvelis-native` を追加。`check-headless-deps` EXACT に
   `gpui`/`ruviform-ui-gpui` 追加。`check-app-native`/`lint-app-
   native` mise task 追加。
6. `tests/binding.rs`/`tests/shell.rs`（`ruviform-ui-test` dev dep）
   で headless 検証。
7. ADR superseded 更新・doc index 再生成（`doc_maintenance.js
   apply`）。
8. `mise run fmt`/`check`/`lint`/`test` + `check-app-native`/
   `lint-app-native` 検証。
9. 独立 review A/B→Validator→Judge 相当→`origin/main` merge→再検証
   →日本語 Conventional Commits→branch push→Refs #175 PR。

## 依存・ブロッカー

- RF387 renderer 候補: 未入手。最小 Tab+Fader slice の接続は
  `NotRun` として記録し、不足契約（mount seam/intent 送出/inbox
  消費）は discovery に固定済み。
- `GpuiDriver` の `ime`=Unsupported、`native_surface_slot`=backend
  依存は正直に capability 出力・受入外。

## 検証マトリクス

| 項 | 手段 | 判定 |
| --- | --- | --- |
| fmt/check/lint/test | `mise run` | 実測 |
| headless 依存境界 | `check-headless-deps` | 実測 |
| shell lifecycle/Binding | `tests/shell.rs`/`binding.rs` (FakeDriver) | 実測 |
| native lane | `check-app-native`/`lint-app-native` | 実測 or NotRun |
| Tab+Fader renderer slice | RF387 接続 | NotRun（候補待ち）|
| 実 OS window/GPU/IME/focus | 実機観測 | NotRun（未観測）|
| plugin dock | RF388/389+SV176 | Out of scope |
