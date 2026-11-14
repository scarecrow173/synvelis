# SF-05 verification results (Issue #108 / synvelis#108)

OS: Windows 11（worktree `sv108-sf05-residual`）。toolchain は
`mise.toml` 管理（Rust stable、edition 2024、CI pin 1.99.0）。
canonical = `mise run` 経由。cargo 直接実行は補助証拠。

変更は `crates/synvelis-workspace/tests/sf_05_s2.rs`（テスト追加）
と文書/evidence のみ。`crates/synvelis-project/`・Ruviform・
Cargo/mise 設定は未変更のため、検証は変更 crate
（synvelis-workspace）と直接影響する下流（synvelis-app）に
比例させた（POLICIES.md §5）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | mise rust stable | 0 | Passed（`cargo fmt --all -- --check`。初回 `sf_05_s2.rs` の整形差分を `cargo fmt -p synvelis-workspace` で修正後に再実行） |
| `mise run check synvelis-workspace` | Windows | 同上 | 0 | Passed（`cargo check --locked --all-targets`） |
| `mise run lint synvelis-workspace` | Windows | 同上 | 0 | Passed（clippy `-D warnings`。初回 `assert_is_empty` 指摘を `assert_eq!(len, 0)` に修正後に再実行） |
| `mise run test synvelis-workspace` | Windows | 同上 | 0 | Passed（全 test target green。`sf_05_s2` 25 tests — 既存 18 + 追加 7 全 pass） |
| `mise run check synvelis-app` | Windows | 同上 | 0 | Passed |
| `mise run lint synvelis-app` | Windows | 同上 | 0 | Passed |
| `mise run test synvelis-app` | Windows | 同上 | 0 | Passed（全 test target green。`sf05_signal_flow`・`sf05_probe_share` 含む） |

## 補助証拠

- `cargo check -p synvelis-workspace --tests`（exit 0）と
  `cargo test -p synvelis-workspace --test sf_05_s2`（25/25 pass）
  を反復確認に使用。canonical は上記 mise タスク。
- `cargo fmt -p synvelis-workspace` を fmt-check 指摘の機械的
  修正に 1 回使用。

## Commit SHA

検証は本 ledger 作成時点の作業ツリー（変更未コミット）で実施。
commit 後の SHA は PR 本文に記載する。

## 未検証（NotRun/Unsupported）

- `*-full`・`*-msrv`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
- 実 OS・実 GPUI Window・実 plugin・実 device・macOS/Linux:
  `external-gates.md` 参照。headless Pass を製品受入へ昇格しない。
- 統合検証: merge 後 main SHA の必須 CI 結果が証拠（Coordinator
  確認）。ローカル検証は変更範囲の検証。
