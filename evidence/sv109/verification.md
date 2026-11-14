# SF-06 cross-gate headless fixture verification (Issue #109 / synvelis#109)

OS: Windows 11 Pro 10.0.26300 build 26300（worktree
`sv109-sf06-fixtures`）。CPU: Intel Core i9-9900K。
FS: NTFS（C:）。toolchain は `mise.toml` 管理（rustc
`1.99.1`、edition 2024、CI pin `1.99.0`）。canonical =
`mise run` 経由。cargo 直接実行は補助証拠。

Synvelis base SHA: `86715f9ac2761f9f275e885e0bbd7ee775d9633e`
（origin/main）。Ruviform resolved SHA:
`fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock`）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows 11 | mise rust 1.99.1 | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-player` | Windows 11 | 同上 | 0 | Passed |
| `mise run lint synvelis-player` | Windows 11 | 同上 | 0 | Passed（clippy `-D warnings`。新規 test の `cast_possible_wrap` を `try_from` へ修正済み） |
| `mise run test synvelis-player` | Windows 11 | 同上 | 0 | Passed（全 test target green。`sf06_send_path_pcm` 3 件含む — `sf06_send_pcm_pre_vs_post_tap`・`sf06_graph_edit_pcm_undo_save_reload`・`sf06_mute_solo_pcm_through_sends`） |
| `mise run test synvelis-project` | Windows 11 | 同上 | 0 | Passed（41 test binaries 全 green — `routing_sf01`・`sf_02`・`sf06_oracle`・`sf04_project_probe`・`sf_xr_01` 等を含む） |
| `mise run test synvelis-render` | Windows 11 | 同上 | 0 | Passed（`l2_e2e` 17・`rcd_04_rt_comp_pcm` 2・`sf03_render` 2・`tim04_plugin_state` 5・`x_e2e` 5 — 全 green） |
| `mise exec -- cargo test --locked -p synvelis-player --test sf06_send_path_pcm`（補助証拠） | Windows 11 | 同上 | 0 | Passed（3 tests・0.07s） |
| `mise exec -- cargo clippy --locked --all-targets -p synvelis-player -- -D warnings`（補助証拠） | Windows 11 | 同上 | 0 | Passed |

## 補助証拠

- `cargo fmt --all`・`cargo clippy`・`cargo test` の直接実行は
  反復中の補助証拠。canonical な記録は上表の `mise run` 行。
- `mise run lint synvelis-player` は package 引数必須の
  `mise.toml` task 経由で実行（clippy `-D warnings`）。

## Expected vs Observed（新規 fixture）

| test | expected | observed | result |
| --- | --- | --- | --- |
| `sf06_send_pcm_pre_vs_post_tap` | master PCM = `1.125·fixture_sample_f32`（clip window）/ `0.0`（外）、全 768 frame・2ch で bit 一致 | `PlayReport.pcm` が期待値と `assert_eq` で全一致 | Pass |
| `sf06_graph_edit_pcm_undo_save_reload` | pre-edit PCM = `1.0·x`、post-edit = `1.75·x`、reload 後同一 PCM/CRC、undo で pre-edit PCM に bit 復帰、intent=revision+1 | 全 assert 一致（revision `rev_before+1`・`pcm`/`crc` 一致） | Pass |
| `sf06_mute_solo_pcm_through_sends` | both audible: 各 window `1.5·x`、mute A: A window `0.0`+B window `1.5·x_B`、solo A: A window `1.5·x_A`+B window `0.0`、全 1024 frame・2ch | 全 assert 一致 | Pass |

## 未検証（NotRun/Unsupported）

- `mise run test-rt`（workspace RT no-alloc/no-block）: 本 slice
  は `synvelis-player` の test 追加のみで RT hot path を変更
  していないため scoped 検証の対象外。既存 `*_rt.rs` は
  `mise run test synvelis-player` で green を確認済み。
- `*-full`・`*-msrv`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
- `check`/`lint` の `synvelis-project`/`synvelis-render`: 本
  slice は同 crate の `src/`・`tests/` を変更していないため
  scoped 検証の対象外（`test` は被覆棚卸しの証拠として実行済み）。
- 実機 audio device・実 plugin・実 OS GUI・consumer: `external-gates.md` 参照。
- merge 後対象 SHA の ci.yml covering run: NotRun pending
  （merge 後の CI 証拠は Coordinator が確認）。
