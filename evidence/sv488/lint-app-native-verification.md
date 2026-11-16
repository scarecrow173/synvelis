# sv488 検証証拠 — Issue #488 lint-app-native clippy::large_enum_variant

- 対象 Issue: <https://github.com/scarecrow173/synvelis/issues/488>
- ブランチ: `fix/488-large-enum-variant`（base: origin/main `f05bdbf`）
- 変更: `crates/synvelis-workspace/src/library_ui.rs` の
  `LibraryAction::ImportSubmit.target` を `Option<ImportTargetWire>`
  → `Option<Box<ImportTargetWire>>` に `Box` 化（`apply()` match arm
  は `target.as_deref()` に整合）。serde wire form・公開シグネチャ・
  derive・`#[non_exhaustive]`・`deny_unknown_fields` は不変。
- 実行環境: Windows worktree（`G:\orca\workspaces\synvelis\
  sv488-ci-fix`）、rustc 1.99.1（mise 管理、`mise.toml` の通常
  toolchain。`mise.ci.toml` pin の 1.99.0 ではない点に注意 —
  AC1 の ubuntu-latest/1.99.0 条件は merge 後の対象 SHA の CI
  証拠に委譲される）。`MISE_ENV=ci` は強制していない
  （POLICIES.md §5）。

## 根本原因の確認（補助証拠 — canonical = mise）

- `cargo tree -p synvelis-app --features native -e features -i
  serde_json`: `serde_json feature "preserve_order"` ← `gpui_perf`
  ← `gpui_util_macros` ← `gpui` ← `ruviform-ui-gpui` ←
  `synvelis-app feature "native"` の経路を確認。`preserve_order`
  で `serde_json::Map` が `IndexMap` 化し `Value` が肥大化する。

## 実行した検証（実測のみ記録）

| コマンド | OS / toolchain | exit | 結果 |
| --- | --- | --- | --- |
| `mise run lint-app-native`（修正前） | Windows / rustc 1.99.1 | 101 | Fail — `clippy::large_enum_variant`（`library_ui.rs:454`、ImportSubmit ≥305B vs ImportRebase ≥80B、enum ≥312B）。CI run 43784707716 と同一 lint・同一サイズ値を再現 |
| `mise run lint synvelis-workspace`（修正前） | Windows / rustc 1.99.1 | 0 | Pass — `preserve_order` 非有効の feature unification では lint 非発火（lane 依存性の確認） |
| `mise run lint-app-native`（修正後） | Windows / rustc 1.99.1 | 0 | Pass — `synvelis-workspace` の lint が `large_enum_variant` なしで完了 |
| `mise run check synvelis-workspace` | Windows / rustc 1.99.1 | 0 | Pass |
| `mise run lint synvelis-workspace` | Windows / rustc 1.99.1 | 0 | Pass |
| `mise run test synvelis-workspace` | Windows / rustc 1.99.1 | 0 | Pass — 全 suite 0 failed（`ImportSubmit` constructor を含む `library_ui::tests` 含む） |
| `mise run test synvelis-app` | Windows / rustc 1.99.1 | 0 | Pass — 47 test binaries 全て 0 failed。`tests/library_browser.rs`（`syn.library.action`/`import.submit` の JSON deserialize 経路）を含む wire 互換の実測 |
| `mise run fmt-check` | Windows / rustc 1.99.1 | 0 | Pass |

## 未検証・委譲項目

- AC1 の `mise.ci.toml` pin 1.99.0・ubuntu-latest 相当での
  `verify (ubuntu-latest)`・`evidence`・`ci-summary` success:
  **NotRun（merge 後 CI 証拠に委譲）**。ローカル検証は変更範囲の
  検証であり、統合受入は merge 後の対象 SHA の CI 証拠で
  Coordinator が確認する（POLICIES.md §5）。
- `*-msrv` 系タスク: **NotRun**（ユーザー指定により実行禁止）。
- `*-full` 系・3OS 横断: **NotRun**（Daily Full CI 委譲、本 slice の
  必須検証に含めない）。
- GUI-L2 / EXT-L3: **NotApplicable**（headless lint 是正であり
  GUI・外部 API を提供/利用しない）。
