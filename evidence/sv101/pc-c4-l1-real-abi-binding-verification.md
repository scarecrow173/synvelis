# PC-C4 L1 real ABI binding — verification record

- 実行日時: 2026-11-26 (Windows)
- base main: `b59333d4` (`test(sv659): representative proof の選択実行を追加 (#670)`、fast-forward 取り込み済み。upstream 差分は docs/tools のみで Rust コード非変更)
- ブランチ: `feat/101-l1-real-abi-binding`
- machine-readable evidence: `evidence/sv101/pc-c4-l1-real-abi-binding.jsonl`（post-merge canonical run から抽出）

## 必須検証コマンド結果（全て mise 経由・正規）

| コマンド | 結果 | exit |
| --- | --- | --- |
| `mise run fmt-check` | Pass | 0 |
| `mise run check synvelis-app` | Pass | 0 |
| `mise run check synvelis-player` | Pass | 0 |
| `mise run check synvelis-plugin-test-l1-runner` | Pass | 0 |
| `mise run lint synvelis-app` | Pass | 0 |
| `mise run lint synvelis-player` | Pass | 0 |
| `mise run test-pc-c4-l1` | Pass（2/2 tests） | 0 |
| `mise run test-tim03-l1` | Pass | 0 |
| `mise run test synvelis-app` | Pass | 0 |
| `mise run test synvelis-player` | Pass（後述の環境 remediation 後に再実行） | 0 |
| `mise run test-rt` | Pass | 0 |
| `mise run check-headless-deps` | Pass | 0 |
| `mise run check-ruviform-rev` | Pass（後述の `sync-ui-deps` 後に再実行） | 0 |
| `mise exec -- prek run --all-files` | Pass | 0 |

## スコープ外 environmental failure の切り分け

### `mise run test synvelis-player` 初回 FAIL → remediation 後 Pass

初回実行（full matrix を同一 shared target dir で順次実行した直後）は
`tim04_l1_abi_state.rs` の 3 test が fail した。assertion 内容は
`target/debug/deps` 内の `ruviform_plugin_test_{vst3,clap}` fixture artifact が
3 つ存在し uniqueness 仮定（left: 3, right: 1）を破るというもの。

切り分け根拠:

- 本 diff は `Cargo.toml`/`Cargo.lock`/synvelis-player 配下を一切変更しておらず、
  依存グラフ・feature unification に影響しない（新規 test file・mise task・docs のみ）。
- 重複 artifact は runner crate build・synvelis-app test build・synvelis-player test
  build が同一 shared target dir で異なる fingerprint の fixture cdylib を生成することに
  由来する環境状態であり、タイムスタンプは全て本 diff のビルド前後の matrix 実行時。
- `cargo clean -p ruviform-plugin-test-vst3 -p ruviform-plugin-test-clap` で stale
  artifact/fingerprint を除去し `mise run test synvelis-player` を再実行したところ、
  本 diff 適用状態のまま全 test pass（exit 0、55 test binaries ok、FAILED なし）。
  再実行後の deps には format ごとちょうど 1 artifact のみ存在した。

この uniqueness 仮定と shared target dir の相互作用は TIM-04（PR #669）側の既存
contract の話題であり、本 slice のスコープ外（tim04 test file は編集禁止範囲）なので
test 本体には触れていない。

### `mise run check-ruviform-rev` 初回 FAIL → `sync-ui-deps` 後 Pass

初回は ui vendor REV `20a24a66…` ≠ lockfile resolved `20dc41f7…` の fail-closed。
原因は worktree 作成時に `.worktreeinclude` で copy された vendor tree の stale
状態であり、本 diff とは無関係。`mise run sync-ui-deps`（synvelis#230 の正規復旧
手順）実行後に `mise run check-ruviform-rev` は Pass（REV match）。
`ui/vendor/ruviform` は gitignore された生成物であり、tracked file への変更は発生
していない。

## PC-C4 実測サマリ（両 format）

`mise run test-pc-c4-l1` exit 0、2/2 pass:

| 項目 | CLAP | VST3 |
| --- | --- | --- |
| activation | Active | Active |
| binding report | bound 1 / unbound 0 / without_control 0 | 同左 |
| frame base | 48000/1（foreign base reject 確認） | 同左 |
| fast write | `call_start=1280, frame_offset=132` Dispatched | 同左 |
| automation | `call_start=5376, frame_offset=36` Dispatched | 同左 |
| PCM unity→half | block 2 → block 8 | block 2 → block 8 |
| `input*0.25`（domain 混同 oracle） | 0 blocks | 0 blocks |
| generation | 1 → 2、retired handles 0 / replacement handles 1 | 同左 |
| stale-generation observe | fail-closed None | 同左 |

staged fixture SHA-256: CLAP `8026ab0b40b8ecbf…`、VST3 `d5c96ce5e8e29577…`。
ruviform resolved revision `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`。

## Known limitations（NotRun を Pass としない）

- sample-exact PCM frame offset は主張しない（frame precision は control ledger
  projection、PCM は value/domain oracle）。
- L1 lookahead 1024 frames の pass-through/substitution は fresh delivery とみなさない。
  production observability から upstream matched/substituted counter は読めず記録しない。
- plugin output report、Frame/Value Receipt RT+Offline parity、typed reject matrix、
  tempo/loop/automation matrix、GUI/EXT-L3、fault/crash/hang、他 fixture は NotRun/後続。
- endpoint detach receipt / same-instance re-attach は upstream 契約不在により未検証。
- ローカル検証は変更範囲の検証であり、統合検証は merge 後の対象 SHA の CI 証拠。
- docs index（`docs/**/README.md`）は未同期のまま。Coordinator が統合時に整備する。
