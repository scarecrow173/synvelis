# SF-04 loop discontinuity verification (Issue #107 / synvelis#107)

Slice: `syn.probe.compare` UI wire OpName + loop→probe 不連続宣言
（SF-PROBE-07 Loop 脚）。branch
`feat/107-probe-compare-opname-loop-discontinuity`。

OS: Windows 11（worktree `sv107-probe-loop`）。toolchain は
`mise.toml` 管理。canonical = `mise run` 経由。cargo 直接実行は
補助証拠。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | mise rust stable | 0 | Passed（`cargo fmt --all -- --check`。初回 `sf04_loop_discontinuity.rs` の rustfmt 差分を `cargo fmt` で修正後に green） |
| `mise run check synvelis-player` | Windows | 同上 | 0 | Passed |
| `mise run lint synvelis-player` | Windows | 同上 | 0 | Passed（clippy `-D warnings`） |
| `mise run test synvelis-player` | Windows | 同上 | 0 | Passed（全 test target green。`sf04_loop_discontinuity` 4 件含む） |
| `mise run test-rt` | Windows | 同上 | 0 | Passed（RT no-alloc/no-block 契約テスト） |
| `mise run check synvelis-app` | Windows | 同上 | 0 | Passed（`engine.rs` 変更を含む `--all-targets`） |
| `mise run lint synvelis-app` | Windows | 同上 | 0 | Passed |
| `mise run test synvelis-app` | Windows | 同上 | 補助 | （結果は PR 本文に記録。既存 app test の非退行確認用・必須 lane 外） |
| `mise run check-ui` | Windows | node/npm | 0 | Passed（`npm ci` + vendor 同期照合 + `tsc --noEmit`。事前に `mise run sync-ui-deps` で stale vendor を lockfile rev `fd1d4d5f` へ再同期 — vendor は gitignore 生成物で tracked 差分なし） |
| `mise run test-ui` | Windows | 同上 | 0 | Passed（vitest 6 files / 56 tests） |
| `mise exec -- prek run --all-files` | Windows | prek | （結果は PR 本文に記録） | pre-commit 全 hook |

## 補助証拠

- `mise exec -- cargo test -p synvelis-player --test
  sf04_loop_discontinuity` → 4 passed / 0 failed（canonical は
  `mise run test synvelis-player`）。
- `node tools/sync-ruviform-ui.mjs` は `mise run sync-ui-deps`
  経由で実行（mise task が node script を呼ぶ薄い wrapper）。

## pin した契約（`sf04_loop_discontinuity.rs`）

- `sf04_loop_wrap_windows_declare_and_close_compare`: loop wrap を
  含む各 block で `observed_loop_discontinuity` → epoch cut
  （`cursor()` 不変）→ wrap/apply 境界を跨ぐ compare が
  `DiscontinuityInWindow` で fail-closed。通常 window は不検出、
  baseline `Aligned` を先に pin。
- `sf04_loop_set_and_clear_apply_declare_without_wrap`:
  未来 loop への `SetLoop` apply（window に wrap 無し）と
  `ClearLoop` apply がそれぞれ epoch cut。開放 capture segment が
  `ClockEpoch` で閉じる。post-clear 通常 window は不検出。
- `sf04_loop_declare_channel_full_keeps_observable_divergence`:
  command channel 満杯で declare が `Broker(CommandChannelFull)`
  を返しても broker epoch は進行 → `probe_time_context` と deck
  mark の乖離が観測可能、drain 後の次 wire で resync。
- `sf04_loop_observe_block_stale_generation_clears_flag`:
  generation 不一致の `observe_block` は `None` + flag `false`
  クリア。stale `true` が次 window に漏れない。

## 未検証（NotRun/Unsupported）

- `*-full`・`*-msrv`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務。
  `*-msrv` はユーザー指定どおり NotRun）。
- 実機 loop wrap・実 device fault・実 plugin: `external-gates.md`
  参照。headless Fixture の成功を実機 Pass にしない。
- `syn.probe.compare` の UI 側呼出し実装・session 露出:
  `scarecrow173/synvelis#108` 系 slice の範囲。本 slice は
  `OpName` union の wire 到達性のみ。
- SF-PROBE-07 Device Restart 脚: engine 再生成 = fresh probe
  context で宣言点不要の既存契約（`routing_engine.rs` doc 明記）。
- 実測 alignment oracle（`scarecrow173/ruviform#309`）:
  NotRun — 契約レベルのみ。
