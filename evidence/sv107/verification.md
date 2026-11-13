# SF-PROBE verification results (Issue #107 / synvelis#107)

OS: Windows 11 (worktree `sv107-probe-adapter`)。toolchain は
`mise.toml` 管理（Rust stable、edition 2024、CI pin 1.99.0）。
canonical = `mise run` 経由。cargo 直接実行は補助証拠。

最終検証は `git merge origin/main`（`8119d3e` 取り込み、clean
merge・競合なし）後の HEAD で実施。merge 差分は synvelis-app の
テスト・docs のみで synvelis-project/synvelis-player に変更なし。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | mise rust stable | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-project` | Windows | 同上 | 0 | Passed |
| `mise run check synvelis-player` | Windows | 同上 | 0 | Passed |
| `mise run lint synvelis-project` | Windows | 同上 | 0 | Passed（clippy `-D warnings`。初回 `doc_markdown` 指摘を修正済み） |
| `mise run lint synvelis-player` | Windows | 同上 | 0 | Passed（同上） |
| `mise run test synvelis-project` | Windows | 同上 | 0 | Passed（全 test target green） |
| `mise run test synvelis-player` | Windows | 同上 | 0 | Passed（全 test target green。`sf04_probe_acceptance` 2 件含む） |
| `mise run test-rt` | Windows | 同上 | 0 | Passed（`cargo test --workspace --locked rt_` — RT no-alloc/no-block 契約テスト） |
| `mise run check-ruviform-rev` | Windows | node | 0 | Passed（`mise run sync-ui-deps` で vendor を lockfile rev `fd1d4d5f` へ再同期後に REV match を確認。vendor は gitignore 生成物で tracked 変更なし） |
| `mise run test synvelis-player --test sf04_probe_acceptance`（merge 後再確認） | Windows | 同上 | 0 | Passed（2 tests） |

## 補助証拠

- `cargo fmt -p synvelis-player` を fmt-check 失敗時の修正に 1 回
  使用（canonical な `mise run fmt` は存在しないため、fmt-check
  の指摘に対する機械的修正として cargo fmt を使用）。
- `node tools/check-ruviform-rev.mjs`・`node tools/
  sync-ruviform-ui.mjs` は `mise run` 経由で実行（mise task が
  node script を呼ぶ薄い wrapper）。

## 未検証（NotRun/Unsupported）

- `*-full`・`*-msrv`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
- 実機 audio device・実 plugin・実 OS レーン: `external-gates.md`
  参照。headless Fixture の成功を実機 Pass にしない。
- GUI（#108）・Domain Oracle（#109）: 本タスク範囲外。
