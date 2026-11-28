# TIM-04 Plugin State quality separation / coverage-aware Strict preflight

本ファイルは `feat/128-plugin-state-recipe` slice の local verification を記録する。
対象 base は `3a7e3ed9b17aebb236acde9bdce125abc721cbe0`、ruviform resolved rev は
`511e1a7944c35797942ba065c092b9263067c48a`、OS/Arch は Windows / x86_64。
local verification は merge 後の統合 CI 証拠ではない。

## 実装判断

- `CaptureMode` は caller の要求であり、achieved quality の根拠ではない。現行 Ruviform
  `SaveState` 観測には plugin/host agreement token がないため、stable non-processing
  boundary は `Quiescent`、processing / unstable boundary は `BestEffort` とする。
  `Coordinated` を観測根拠なしに返さない。
- quality summary の authoritative rule は `synvelis-project` に置く。`exact_eligible` は
  件数の内部整合を要求し、`exact_eligible_for_processors` は project の processor coverage
  との一致まで要求する。zero-count summary は processor-free project にだけ有効である。
- persisted captured record に binding identity がないものは projection 構築時に fail-closed
  とする。binding の descriptor/version/capability revision が現行 processor と一致しない
  evidence は exact eligibility に使わない。host runtime instance / graph generation /
  project revision / stored_at は provenance であり、acoustic identity へ昇格させない。
- serialized manifest shape、canonical recipe version、recipe schema version は変更しない。
  `UnknownCause` には新 variant を追加しない。stale binding は現行 identity に対する
  plugin-state evidence がないものとして typed `PluginStateUnknown` に倒す。

## 検証コマンド

| command | result | observed |
| --- | --- | --- |
| `mise run check synvelis-project` | Pass | exit code 0 |
| `mise run lint synvelis-project` | Pass | exit code 0 |
| `mise run test synvelis-project --test plugin_state` | Pass | exit code 0、20 passed / 0 failed |
| `mise run check synvelis-player` | Pass | exit code 0 |
| `mise run lint synvelis-player` | Pass | exit code 0 |
| `mise run test synvelis-player --test tim04_plugin_state` | Pass | exit code 0、10 passed / 0 failed |
| `mise run check synvelis-render` | Pass | exit code 0 |
| `mise run lint synvelis-render` | Pass | exit code 0 |
| `mise run test synvelis-render` | Pass | exit code 0。TIM-04 render tests 6/6、RCP-01 projection tests 10/10 を含む |
| `mise run test-rt` | Fail（scope外・並行 timing） | exit code 101。`synvelis-app` `library_browser` の import job 6 件が timeout。補助検証 `cargo test -p synvelis-app --test library_browser rt_ -- --test-threads=1` は exit code 0、8 passed / 0 failed。plugin-state 差分とは無関係の app import lane |
| `mise run fmt-check` | Fail（環境制約） | exit code 1。cargo fmt が workspace 全 file を 1 つの Windows command line に連結し、長い Orca worktree path で `os error 206` となった。コード差分に起因しない |
| scoped rustfmt（変更 file 10 件） | Pass | `rustfmt --edition 2024` exit code 0 |

## 回帰観点

- player headless test は processing / stopped 両方で `CaptureMode` を変えても achieved quality
  と lifecycle observation が変わらないことを確認する。stopped boundary は `Coordinated` 要求でも
  `Quiescent`、processing は `BestEffort` である。
- project test は exact coverage、missing coverage、zero-count vacuous truth、内部不一致 count を
  直接検証する。Strict save は exact quality で publish し、degraded quality では publish しない。copy restore が record quality と digest を維持する既存回帰も同じ test binary で Pass。
- render test は Strict offline export が project processor 数と矛盾する evidence、coverage 不足、
  count 不整合を output byte 作成前に拒否することを確認する。processor-free project の zero evidence
  は valid な empty coverage として扱う。
- RCP-01 projection test は missing/corrupt blob、manifest failure、missing evidence、
  BestEffort/Unsupported/Failed、captured record の binding 欠落、stale plugin identity を
  fail-closed で確認する。runtime instance / graph generation の差は acoustic identity に影響しない。
  verified artifact は non-resolved resolution へ昇格しない。

## sv#655 lease により残る player 実 ABI / L1 期待値更新

`crates/synvelis-player/tests/tim04_l1_abi_state.rs` と
`crates/synvelis-player/tests/tim04_real_abi_state.rs` は sv#655 lease のため変更していない。
品質分離の root-cause 修正に合わせるため、sv#655 統合後の follow-up で少なくとも次の期待値更新が必要である。

- `tim04_real_abi_state.rs` の `expected_quality(processing, mode)` は、stopped boundaryでは
  mode に関係なく `StateCaptureQuality::Quiescent` を返す。`mode` 引数は観測へ使わない。
- `tim04_l1_abi_state.rs` の stable stopped capture assertion は `Coordinated` ではなく
  `StateCaptureQuality::Quiescent` を期待する。evidence detail の `captures Coordinated` 表記も
  `captures Quiescent` に合わせる。

これらの実 ABI / L1 tests は本 slice では実行していない（NotRun）。未更新の期待値が残るため、
実行すると stable stopped `Coordinated` 期待との差異で Fail になる可能性が高い。PCM oracle、
3OS、macOS/Linux、GUI、VCS、AudioLens、市販 plugin、実 runtime endpoint も本 slice では NotRun。
