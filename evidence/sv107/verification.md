# SF-PROBE verification results (Issue #107 / synvelis#107)

OS: Windows (worktree `sv107-probe-adapter`)。toolchain は
`mise.toml` 管理（Rust stable、edition 2024）。canonical =
`mise run` 経由。cargo 直接実行は補助証拠。

## 実行結果

| コマンド | 結果 | 備考 |
| --- | --- | --- |
| `mise run test synvelis-player --test sf04_probe_acceptance` | exit 0 / 2 passed | 新規テスト。初回実行時に未使用 helper の warning が出たため削除して再実行（最終状態は warning なしで再確認予定） |

（fmt-check / check / lint / test（両 crate）/ test-rt /
check-ruviform-rev は最終検証で追記）

## 補助証拠

- `mise run test synvelis-player --test sf04_probe_acceptance`
  は `mise run test` の package+test-filter 経路で実行。
  `cargo test` 直接実行は行っていない。

## 未検証

- `mise run fmt-check` / `check` / `lint` / `test`（synvelis-
  project・synvelis-player 全体）/ `test-rt` / `check-ruviform-rev`:
  最終検証フェーズで実行予定。結果は本ファイルに追記する。
- `check-ruviform-rev`: 事前監査で vendor REV
  `20a24a66...` vs lockfile `fd1d4d5f...` の不一致を確認済み。
  `mise run sync-ui-deps` は `ui/vendor`（gitignore 生成物）を
  更新するが、本タスクの編集範囲外ファイルの変更を伴う可能性が
  あるため、実行可否は範囲確認の上で判断する。
