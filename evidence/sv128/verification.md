# Verification — synvelis#128 TIM-04 real ABI Plugin State Snapshot slice

本ファイルは local verification laneの結果を記録する。real ABI caseごとの
仕様証拠・artifact hash・capability matrix・round-trip oracleは
`real-abi-state.md` と同 directoryの `*.jsonl` を参照する。

## 環境

- branch: `scarecrow173/sv128-tim04-real-abi`
- base: dispatch 時点の `origin/main` `af159f30`
- OS/Arch: Windows / x86_64
- toolchain: rustc 1.99.1 (`x86_64-pc-windows-msvc`、mise管理)
- ruviform resolved rev: `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
- raw console log: local worktreeの `verification-local.log`（`.gitignore` の
  `*.log` 規則により tracked evidenceではない。以下に command・exit code・
  集計を記録する）

## Lane results

| command | result | observed |
| --- | --- | --- |
| `mise run fmt-check` | Pass | exit code 0 |
| `mise run check synvelis-player` | Pass | exit code 0（`cargo check --locked --all-targets`） |
| `mise run lint synvelis-player` | Pass | exit code 0（`cargo clippy --locked --all-targets -- -D warnings`） |
| `mise run test synvelis-player` | Pass | exit code 0。51 test binaries、361 passed / 0 failed / 0 ignored |
| `mise run test-rt` | Pass | exit code 0。323 test binaries、filter `rt_` で 161 passed / 0 failed / 0 ignored（3435 filtered out） |
| `mise run check-headless-deps` | Pass | exit code 0。`no GUI framework deps in 12 headless crates` |
| `mise run check-ruviform-rev` | Pass | exit code 0。`resolved 20dc41f71c47674b3e51d1eb0921124b94f8bfaf (36 lockfile sources, lockfile-primary, REV match)` |

`mise run test synvelis-player` のうち、本 sliceの
`tests/tim04_real_abi_state.rs` は **6 passed / 0 failed**。既存
`tests/tim04_plugin_state.rs` も 9 passed / 0 failedで、headless/mock
sliceの非回帰を確認した。

## Evidence generation run

証拠生成を兼ねた限定実行:

```sh
SYNVELIS_TIM04_REAL_ABI_EVIDENCE_DIR=evidence/sv128 mise run test synvelis-player --test tim04_real_abi_state
```

結果: exit code 0、6/6 tests Pass。machine-readable rowsは43件で、
verdict集計は **Pass 29 / Unsupported 14 / Fail 0 / NotRun 0**。
`Unsupported` 14件の内訳は stateless `test_fault` capture matrix 12件と、
VST3/CLAP typed runtime endpoint probe 2件。endpoint probeの実測 errorは
`EndpointUnavailable`、発生地点は `PluginHost::prepare_endpoint`、factory
capabilityは `create_sendable=false` だった。

## Local-only notes

- 初回 `check-ruviform-rev` は、この worktreeへ copyされていた ignored
  generated vendor treeが base時点の ruviform rev `20a24a66...` で stale
  だったため失敗した。AGENTS.mdの指示どおり `mise run sync-ui-deps` を実行し、
  lockfile rev `20dc41f71c...` へ再同期した後の最終 laneは Pass。vendor treeは
  gitignoreされた生成物であり、tracked diffには含めていない。
- 上表は local verificationであり、CI runの証拠ではない。PRでは localPassと
  CI結果を区別する。

## Scope / non-claims

- Windows x86_64・in-process/L0・pinned ruviform revの実ABI Test Pluginに
  対する consumer-run。市販 plugin、AU、3OS横断、実 GUI/Native Editor、
  物理機材、AudioLens cross-gateは対象外。
- typed runtime endpointは現行 revで `Unsupported`。PCM oracleは manual
  production `PluginHost::process`経路であり、typed graph attach成功を意味
  しない。
- `test_fault` の abort/exit/hang fault bandは in-process runnerを殺すため
  本 sliceでは armしていない。
