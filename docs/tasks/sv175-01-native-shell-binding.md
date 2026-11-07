---
id: "TASK-hSG0A7xXhnXVL7exOdhVuV"
type: "task"
status: "done"
title: "SV175-01: NativeShell・ViewBinding・native feature lane 実装"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv175-native-shell.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/designs/sv175-native-shell-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv175-01-native-shell-binding.md"
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

# SV175-01 実装タスク

## 変更点

- `crates/synvelis-app/src/binding.rs` 新設: `ViewBinding`/`BindingNotice`/
  `ProjectionRow`。
- `crates/synvelis-app/src/shell.rs` 新設: `NativeShell`。
- `crates/synvelis-app/src/lib.rs`: `ui_clock()` accessor + cell 統一、
  `binding`/`shell` module 公開。
- `crates/synvelis-app/Cargo.toml`: `ruviform-ui-gpui` optional dep、
  `native` feature、`[[bin]] synvelis-native`、dev dep
  `ruviform-ui-test`。
- `crates/synvelis-app/src/bin/synvelis-native.rs` 新設。
- `tools/check-headless-deps.mjs`: EXACT に `gpui`/`ruviform-ui-gpui`
  （必要なら `ruviform-ui-webview`/`wry`/`tao` も）追加。
- `mise.toml`: `check-app-native`/`lint-app-native`。
- `Cargo.lock`: ruviform `e8adcc09` 系へ再解決。
- `tests/binding.rs`/`tests/shell.rs` 新設。
- `docs/adr/sv-gui-01-…` → status `superseded` + `superseded-by` 本
  ADR。docs index 再生成。

## 完了条件

- spec の Acceptance Criteria 全項。
- `mise run fmt`/`check`/`lint`/`test` green。
- `check-app-native`/`lint-app-native` 実測（環境制約なら NotRun
  +理由）。
- Tab+Fader slice は RF387 待ちで `NotRun` 明記。
