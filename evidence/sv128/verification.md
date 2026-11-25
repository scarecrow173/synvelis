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

## pstack interrogate review

対象は review開始時点の PR diff `af159f30eef33a74220a2c45614deb7d4e6278d5..
8df68b3b8f4dac281923da4fba048e5a7b784560`。pstack `interrogate` handoffで、
correctness-criticalな RFST v2 parser（当初 `tim04_real_abi_state.rs:533-656`
付近）と capability matrix分類（当初 `:920-1028` 付近）を明示対象にした
adversarial reviewを実行した。設定された reviewer seatを起動し、長時間応答の
なかった seatは time-boxed replacement seatで補完した。各 reviewerは実走で
証明できない指摘を残さないルールで報告した。

| reviewer seat | result | dropped |
| --- | --- | --- |
| `ea68ad72` | major artifact provenance finding | 6 |
| `b20793eb` | 同一 root causeの major finding | 5 |
| `edaa3e6a` | 同一 root causeの major finding | 3 |
| `d1a7d305` | additional verified findingなし | 4 |
| `2f722526` | 同一 root causeの major finding | 4 |
| `728d08d7` | 同一 root causeの major finding | 4 |
| `99e5e49e` | cutoff時点で reportなし（dropout） | — |

重複をmergeした verified findingは **major 1件**、blocker 0件だった。
`crates/synvelis-player/tests/tim04_real_abi_state.rs` の artifact resolverが
un-hashed/lexicographically first hashed DLLを provenance確認なしに採用し、
shared target dirに残った stale/foreign/modified fixtureを選んでも locked
ruviform rev付きの `Pass` evidenceを出せることが、複数 reviewerの再現と
parentの再検証で示された。例えば foreign ruviform worktree由来の un-hashed
VST3 DLLを置くと、元実装は `artifact_resolution_and_scan_surface_fail_closed`
を Passにし、SHA-256 `fb84fae9cfd73beafe65d09e605d5b9394ab66a5799433d84302fcf4c0b56a3e`
の DLLと `ruviform_resolved_rev=20dc41f71c...` を同じ evidence rowへ記録した。

RFST v2 parserと capability matrixには verified findingがなかった。複数
reviewerが canonical RFST/VST3/CLAP byte列・FNV-1a・SHA-256を independent
Python oracleで再構成し、committed round-trip evidenceの hashと一致することを
確認した。matrix rowの processing/stable-boundary observationと achieved
qualityも矛盾しなかった。v1/degraded extension受入や distributed controller
normalized `0.5`維持などの疑義は、この testが canonical captured outputだけを
検証する oracleであること、upstream fixture仕様と docs開示を理由に dropped
された。

対応として、implicit artifact candidateに sibling dep-info provenance filterを
追加した。filterは dep-infoが cargo git checkout配下の対象 fixture crate
sourceを参照し、その checkout revisionが `Cargo.lock` の ruviform resolved
revに対応することを要求する。dep-info欠落・別 rev・別 worktree由来の
candidateは採用しない。positive/negative dep-info probeを
`artifact_resolution_and_scan_surface_fail_closed` に追加し、Linux/macOSの
`lib` prefixにも対応した。

修正後の adversarial validationでは、valid locked artifactを `*-zzz.dll`、
1 byte改変した lexicographically earlier artifactを wrong-rev dep-info付き
`*-aaa.dll` として混在させた。次の実行で artifact testは Passし、改変側では
なく locked側が選択された。

```sh
SYNVELIS_TIM04_REAL_ABI_EVIDENCE_DIR='C:\Users\akiyoshi\AppData\Local\Temp\tim04-artifact-fix-proof\evidence' \
  /tmp/tim04-artifact-fix-proof/deps/tim04_real_abi_state.exe \
  artifact_resolution_and_scan_surface_fail_closed --exact --nocapture
```

observed selected artifacts:

- VST3 `ruviform_plugin_test_vst3-zzz.dll` / SHA-256 `f8ff6d418c410c1ee599ab5f212235572d7f35c8122df8b9a2a6dbffbb4927b6` / Pass
- CLAP `ruviform_plugin_test_clap-zzz.dll` / SHA-256 `3330419055b034d5118d0a2139c223a5e63bfff17063458d6bd877aa9e177742` / Pass

最初の reviewerへ修正後レビューを返したところ、foreign un-hashed artifactに
sibling dep-infoがある場合でも rejectionされ locked artifactが選択されることを
独立に再実行確認し、remaining blocker/majorなしの verdictだった。explicit
path overrideは運用者明示の trusted escape hatchとして残る。最終 verdictは
**fixed major 1件、remaining blocker/major 0件**。

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
