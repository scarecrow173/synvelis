# PC-C6 acceptance ledger verification (Issue #103 / synvelis#103)

OS: Windows 11 Pro 10.0.26300（worktree
`sv103-pc-c6-ledger`）。CPU: Intel Core i9-9900K。
FS: NTFS（C:）。toolchain は `mise.toml` 管理（rustc
`1.99.1`、edition 2024、CI pin `1.99.0`）。canonical =
`mise run` 経由。cargo 直接実行は補助証拠。

Synvelis base SHA: `eac70aca419508c9837fa18a03f4f75223e52749`
（origin/main HEAD）。Ruviform resolved SHA:
`fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock`）。

本 slice は docs/evidence のみ（新規 test fixture 追加なし —
`acceptance-ledger.md`「fixture 補完判定」参照）。検証は
ledger が引用する headless fixture の実測 green を確認する
ために実施した。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows 11 | mise rust 1.99.1 | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-project` | Windows 11 | 同上 | 0 | Passed |
| `mise run check synvelis-player` | Windows 11 | 同上 | 0 | Passed |
| `mise run lint synvelis-project` | Windows 11 | 同上 | 0 | Passed（clippy `-D warnings`） |
| `mise run lint synvelis-player` | Windows 11 | 同上 | 0 | Passed（clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | 同上 | 0 | Passed（全 test binary green — `pc_c6_oracle_rel` 7・`pc_c6_oracle_tr` 8・`pc_c2_relation` 5・`trace_c3` 6・`aut_02` 15・`controller_store` 11・`plugin_state`・`pc_b_rt` 等を含む） |
| `mise run test synvelis-player` | Windows 11 | 同上 | 0 | Passed（全 test binary green — `pc_c6_oracle_pl` 8・`pc_c6_oracle_pl_rt` 1・`pc_c6_oracle_tr_dispatch` 8・`trace_c3_05` 5・`trace_c3_05_rt` 1・`pc_c4_*`・`tim03_*`・`tim04_plugin_state` 9・`pc_b` 8 等を含む） |
| `mise run test synvelis-workspace` | Windows 11 | 同上 | 0 | Passed（補助 — ledger が引用する oracle fixture の実測確認。`pc_c6_oracle_rec` 8・`pc_c6_oracle_ui` 7・`pc_c1`・`aut_01` 15・`aut_03` 19・`aut_04`・`aut_04_rt`・`pc_a`・`uic5` 14・`uic5_04` 10・`rel_c5` 11・`sv47_controls` 25・`sf_05`/`sf_05_s2` 等を含む） |

## 補助証拠

- `cargo fmt`/`clippy`/`test` の直接実行は反復中の補助証拠。
  canonical な記録は上表の `mise run` 行。
- `mise run test synvelis-workspace` は task 指定の正規検証
  コマンド一覧外だが、ledger が `synvelis-workspace/tests/`
  の fixture を Pass-measured の根拠として引用するため、
  引用の正直性のために実行した（変更範囲に比例した検証 —
  POLICIES.md §5）。
- `mise run check`/`lint` の `synvelis-workspace` は本 slice が
  同 crate を変更していないため未実行（`test` のみ証拠引用の
  ため実行）。

## Expected vs Observed（引用 fixture の代表例）

| fixture | expected | observed | result |
| --- | --- | --- | --- |
| `pc_c6_oracle_rec.rs` 8 tests | record_start fixed context・CAS/stale 点列保持・1 Undo・rebase・LoopWrap・mode gating・scope drift が宣言モデルと field 一致 | 全 assert 一致（8/8 pass） | Pass |
| `pc_c6_oracle_rel.rs` 7 tests | REL01〜07 が宣言 edge 集合の独立 BFS 導出と一致 | 全 assert 一致（7/7 pass） | Pass |
| `pc_c6_oracle_tr.rs` 8 tests | TR01〜08 の stage/stale/provenance/silence/bounded が宣言と一致 | 全 assert 一致（8/8 pass） | Pass |
| `pc_c6_oracle_tr_dispatch.rs` 8 tests | 実 ledger 経路の TR01/02/03/04/05/06/07/08 が oracle 導出と field 一致 | 全 assert 一致（8/8 pass） | Pass |
| `pc_c6_oracle_pl.rs` 8 tests + `pc_c6_oracle_pl_rt.rs` | PL01〜08 の pin/typed reject/QueueFull/atomic/min-frame-offset/zero-misdelivery/boundary が宣言と一致・warm hot path alloc=0 | 全 assert 一致（9/9 pass） | Pass |
| `pc_c6_oracle_ui.rs` 7 tests | UI01〜07 の relation view/nav/stale/2window/lifecycle/boundary marker が宣言と一致 | 全 assert 一致（7/7 pass） | Pass |

## 未検証（NotRun/Unsupported）

- `mise run test-rt`（workspace RT no-alloc/no-block）: 本 slice
  は docs/evidence のみで RT hot path を変更していないため
  scoped 検証の対象外。既存 `*_rt.rs` は `mise run test` で
  green を確認済み。
- `*-full`・`*-msrv`・Feature Matrix・3OS 横断・Golden/長時間
  回帰: 日常検証の対象外（Daily/Manual Full CI の責務）。
  ユーザー指定により MSRV 系タスクは実行しない。
- `check`/`lint` の `synvelis-workspace`: 同上（変更なし）。
- 実機 audio device・実 plugin（VST3/CLAP/AU）・実 OS GUI・
  両 consumer: `external-gates.md` 参照 — 全て NotRun/
  external owner。
- 絶対性能閾値（REL07 大規模 graph・X5 recorded points perf・
  PL/TR の RT latency）: baseline 未確定のため NotRun
  （sanity 上限/alloc-free のみ計測済み）。
- merge 後対象 SHA の CI covering run: NotRun pending
  （merge 後の CI 証拠は Coordinator が確認）。
