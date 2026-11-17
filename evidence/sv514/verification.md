# sv514 verification — 検証実測記録

Issue: https://github.com/scarecrow173/synvelis/issues/514
Date: 2026-11-18
OS: Windows (worktree `G:\orca\workspaces\synvelis\sv514-session-engine-selection`)
Toolchain: mise lock（rustc 1.99.1 / rustfmt 1.10.0）

## 実行結果

| コマンド | exit | 結果 | 備考 |
| --- | --- | --- | --- |
| `mise run fmt-check` | task failed | **Fail（環境起因・既存）** | `cargo fmt --all -- --check` が os error 206（filename too long）で失敗。clean tree（`git stash` 検証）でも同一失敗 → 本変更と無関係の既存環境問題。補助証拠: 全 15 workspace package を `cargo fmt -p <pkg> -- --check` で個別検証し全て pass |
| `mise run check synvelis-app` | 0 | Pass | |
| `mise run lint synvelis-app` | 0 | Pass | `-D warnings` で warning なし |
| `mise run test synvelis-app` | 0 | Pass | headless/fixture lane。全 suite `test result: ok`・0 failed（`sf03_session_engine` 含む） |
| `mise run check synvelis-workspace` | 0 | Pass | |
| `mise run lint synvelis-workspace` | 0 | Pass | `-D warnings` で warning なし |
| `mise run test synvelis-workspace` | 0 | Pass | `rcp_selection_integration` 10/10 pass（`rcp_ex05_stale_selection_never_publishes` 含む）・全 suite 0 failed |

## NotRun / Unsupported

| lane | 状態 | 理由 |
| --- | --- | --- |
| T3/T4 実 device/plugin・実 OS・RT/PCM 実測 | **NotRun** | gate: #109・#296・`scarecrow173/ruviform#630`・`scarecrow173/ruviform#631`。headless Pass を実機 Pass に昇格しない |
| `mise run check-headless-deps` | **NotRun** | dep 追加なしのため対象外（dep 追加時のみ必須） |
| `mise exec -- prek run --all-files` | **NotRun** | dep 追加なしのため対象外 |
| `check-full`/`lint-full`/`test-full`/`build-full`/`verify-full`・`test-msrv`・webview/native/gpui lane | **NotRun** | 本 slice の必須検証に含めない（Daily Full CI・別 lane 委譲） |
| 統合検証（merge 後 main SHA の必須 CI） | **Pending** | ローカル検証は変更範囲の検証。統合受入は merge 後 CI 証拠（Coordinator 確認） |

## 補助証拠（canonical = mise、cargo 直接実行は補助）

- `cargo fmt -p <pkg> -- --check` × 15 package: 全て pass
  （`mise run fmt-check` の os error 206 回避のため個別実行。
  `cargo fmt --all` が clean tree でも失敗する既存環境問題の切り分け）
- `git stash` + `cargo fmt --all -- --check` で clean tree でも
  同一失敗を確認（既存環境問題の実証）
