# TIM-04 ABI quality expectation 修正 evidence

本ファイルは `fix/128-tim04-abi-quality` slice の local verification と branch CI 証跡を記録する。Issue #128 全体の受入または close を意味しない。local verification は変更範囲の検証であり、merge 後の対象 SHA に対する必須 CI 結果が統合検証の証拠である。

## 対象 SHA と環境

- 初期 base: `c5e86b6e71cd6ede25ff69feaa8989cdaf8688e9`（PR #699 merge 後の `origin/main`）
- 最終検証前に取り込んだ `origin/main`: `88ec448e16b5cf868f73c8af4f50902c73b079da`（PR #698）
- code head / local verification 対象: `82c06e1155771169f84a26f367df2b608def07a7`
- ruviform resolved rev: `511e1a7944c35797942ba065c092b9263067c48a`
- OS/Arch: Windows / x86_64（`MINGW64_NT-10.0-26300`）
- toolchain: rustc 1.99.1 (`58cf8f9c3 2026-10-13`)、cargo 1.99.1、mise 2026.11.2
- 変更 code 対象: `crates/synvelis-player/tests/tim04_l1_abi_state.rs` と `crates/synvelis-player/tests/tim04_real_abi_state.rs`

## 修正判断

PR #699 後の production implementation は、要求された `CaptureMode` を achieved quality の根拠に使わない。`PluginStateHost::capture` は lifecycle と stable boundary を観測し、stable non-processing boundary を `Quiescent`、processing / unstable boundary を `BestEffort` とする。host/plugin agreement point を確認する仕組みがない限り `Coordinated` を返さない。

したがって今回の根本原因は implementation regression ではなく、artifact-dependent test 側の stale oracle だった。production source は変更していない。L1 の stable stopped capture、roundtrip 入力、permission matrix 入力は `Quiescent` を期待し、real ABI matrix の `expected_quality` は requested mode に依存せず processing の有無だけで決まる。

`Coordinated` を要求する case は残っている。これは要求 mode を網羅するためであり、観測 quality が要求によって昇格することを意味しない。static audit でも stale な `StateCaptureQuality::Coordinated` expectation は残っていないことを確認した。

## 修正前の再現

`c5e86b6e71cd6ede25ff69feaa8989cdaf8688e9` 相当で修正前の dedicated lane を実行した。

- `mise run test-tim04-l1`: exit code 101。3 passed / 1 failed。`l1_scan_lifecycle_and_capture_quality_matrix` が stable stopped capture で実測 `Quiescent`、期待 `Coordinated` となった。
- `mise run test-tim04-real-abi`: exit code 101。5 passed / 1 failed。`capability_matrix_is_measured_for_real_test_plugins` の `vst3-matrix-test_stateful-stopped-Coordinated` が `Pass` 期待に対して `Fail` となった。

## 検証コマンドと結果

最終 code head `82c06e1155771169f84a26f367df2b608def07a7` で実行した。証拠生成を伴う dedicated lane でも同じ mise task を使い、出力先だけを環境変数で指定した。

| command | result | observed |
| --- | --- | --- |
| `mise run fmt-check` | Fail（環境制約） | exit code 1。`cargo fmt --all -- --check` が Windows command line 長の制限で `os error 206` となった。コード差分に起因しない |
| `cargo fmt -p synvelis-player -- --check` | Pass（補助証拠。canonical = mise） | exit code 0 |
| `mise run check synvelis-player` | Pass | exit code 0 |
| `mise run lint synvelis-player` | Pass | exit code 0 |
| `mise run test synvelis-player` | Pass | exit code 0。56 result blocks、379 passed / 0 failed / 10 ignored |
| `mise run test-rt` | Pass | exit code 0。342 result blocks、173 passed / 0 failed / 6 ignored / 3515 filtered out。filter 外の non-RT tests は実行されていない |
| `SYNVELIS_TIM04_L1_EVIDENCE_DIR=G:/orca/workspaces/synvelis/sv128-tim04-abi-quality/evidence/sv128/tim04-abi-quality mise run test-tim04-l1` | Pass | exit code 0。4/4 tests passed |
| `SYNVELIS_TIM04_REAL_ABI_EVIDENCE_DIR=G:/orca/workspaces/synvelis/sv128-tim04-abi-quality/evidence/sv128/tim04-abi-quality mise run test-tim04-real-abi` | Pass | exit code 0。6/6 tests passed |

raw console log は repository の `*.log` 規則により tracked evidence に含めていない。上表の exit code と集計は local worktree に残る raw log から確認した。

## Artifact isolation

`mise run test synvelis-player` では `tim04_l1_abi_state` の 4 tests が dedicated L1 lane 向け理由付き `ignored`、`tim04_real_abi_state` の 6 tests が dedicated real ABI lane 向け理由付き `ignored` だった。generic lane で artifact-dependent test は実行されていない。

dedicated lane はそれぞれ `--ignored` を指定して 4/4・6/6 を実行した。ignore の削除、generic lane への復帰、artifact 不在時の偽 Pass、ignore 範囲の拡大は行っていない。

## Machine-readable evidence

現行 code head に対応する rows は `evidence/sv128/tim04-abi-quality/` の 44 files に記録した。ruviform resolved rev はすべて `511e1a7944c35797942ba065c092b9263067c48a` である。

- L1: `tim04_l1_abi_state.jsonl` に 44 rows。verdict は Pass 40 / Unsupported 4 / Fail 0 / NotRun 0。VST3/CLAP の stable stopped capture はどちらも `Quiescent`。
- real ABI capability matrix: 30 rows。verdict は Pass 18 / Unsupported 12 / Fail 0 / NotRun 0。stateful plugin の processing capture 9 rows は expected/achieved とも `BestEffort`、stopped capture 9 rows は expected/achieved とも `Quiescent`。stateless `test_fault` の 12 rows は state extension がないため `Unsupported`。
- artifact/scan: VST3/CLAP とも exact test-plugin cdylib を解決し、production host scan surface で required fixture descriptor を観測した。VST3 artifact SHA-256 は `2fa977c4ec8fec7da92f9f69585d41491989374e431c44a25ddb32a1ff8d8c42`、CLAP artifact SHA-256 は `4c369db0e0d7144acf6e767664a25987b7a1186c8eead660f8b08870c5094cc0`。
- roundtrip: VST3 distributed / VST3 single / CLAP stateful の 3 cases が Pass。save → mutation → restore → save で blob hash と deterministic PCM oracle を照合した。
- blob integrity: corrupt / missing blob の 4 cases が Pass。typed `Corrupt` / `Missing` で restore port へ壊れた blob を渡さない。
- typed runtime endpoint: VST3/CLAP とも `create_sendable=false` により `EndpointUnavailable` で `Unsupported`。これは capability の正直な観測であり、PCM endpoint 成功の証拠ではない。

`evidence/sv128/` 直下の旧 `*-matrix-*.jsonl` と `tim04_l1_abi_state.jsonl` は、ruviform rev `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` の旧 slice に対する historical evidence である。現行 head の quality 判定にはこの subsection の `tim04-abi-quality/` を使う。

## Branch CI

`ci` workflow は manual `workflow_dispatch` のため、code head `82c06e1155771169f84a26f367df2b608def07a7` を `expected_sha` に指定して dispatch した。

- run: https://github.com/scarecrow173/synvelis/actions/runs/46131396687
- target SHA / `expected_sha`: `82c06e1155771169f84a26f367df2b608def07a7`
- status: completed / success（updated 2026-11-28T10:13:05Z、`gh run watch 46131396687 --exit-status` exit code 0）
- jobs: `verify` ubuntu/windows、`rt-verify`、`msrv-verify`、`gw10-verify` ubuntu/windows、`ui-verify` ubuntu/windows、`dep-guard`、`evidence`、`ci-summary` が success。観測対象の macOS legs (`verify-macos` / `gw10-verify-macos` / `ui-verify-macos`) も success

pre-merge code head `5b6fb9fbb36a22af872ac2ac88b1aba5356ec3e5` を対象にした先行 run https://github.com/scarecrow173/synvelis/actions/runs/46130695432 は、最終 code head の dispatch により canceled となった。これは最終検証証拠に使わない。

この evidence を記録する commit は code head の evidence-only descendant である。code head、CI target、最終 branch head の関係は PR 本文にも記載する。

## Non-claims

- 本 slice は Issue #128 全体の受入ではない。実 OS window / GUI、市販 plugin、物理機材、AudioLens consumer、offline render 実組込み、VCS/Undo 全体、macOS/Linux での GUI-L2/EXT-L3 は対象外または NotRun である。
- `BestEffort` capture を exact / BitExact と主張しない。`Unsupported` を失敗や Pass に読み替えない。
- stable stopped capture の `Quiescent` は plugin/host agreement point を確認した `Coordinated` ではない。現行 Ruviform rev にその agreement token がないため、`Coordinated` の達成を主張しない。
- typed runtime endpoint の `Unsupported` は ruviform 側 capability gap の観測であり、本 slice で解消していない。
- branch CI は branch head の検証であり、merge 後 main SHA の統合受入証拠ではない。
