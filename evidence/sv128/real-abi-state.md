# Evidence — synvelis#128 TIM-04 real ABI Plugin State Snapshot slice

Issue #128（TIM-04: Plugin State Snapshot）のうち、mock/fake pluginに
依存しない **real ABI consumer-run** sliceの証拠。ruviform#630 の正規 ABI
Test Plugin cdylibを VST3/CLAP でロードし、production `PluginHost` cell、
`PluginStateHost`、`PluginStateSession`、`DirectoryPluginStateStore` を通して
state capture/save/restoreを実走した。

本 slice は Issue #128 全体の受入ではない。対象は Windows x86_64・
in-process/L0・ruviform resolved rev
`20dc41f71c47674b3e51d1eb0921124b94f8bfaf` に対する consumer-runである。

## Anchor — 対象 SHA・環境

- branch: `scarecrow173/sv128-tim04-real-abi`
- base: dispatch 時点の `origin/main` `af159f30`
- worktree: `G:\orca\workspaces\synvelis\sv128-tim04-real-abi`
- toolchain: rustc 1.99.1 (`x86_64-pc-windows-msvc`、mise管理)
- OS/Arch: Windows / x86_64
- isolation: in-process/L0
- environment kind: 実ABI Test Plugin（`environment_kind=test-plugin`）
- ruviform resolved rev: `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
- test implementation: `crates/synvelis-player/tests/tim04_real_abi_state.rs`
- machine-readable evidence: この directoryの `*.jsonl`

変更面は `crates/synvelis-player/Cargo.toml` の dev-dependency edge、
`Cargo.lock` の必要最小限更新、test、docs chain、evidence。
`crates/synvelis-player/src/` を含む production sourceの変更はない。

## Artifact identity

| format | artifact | SHA-256 | required scan |
| --- | --- | --- | --- |
| VST3 | `ruviform_plugin_test_vst3-1ac2c390d2654dc3.dll` | `0975e9de4009781ebaa3b10637240a602cc92ec5ef68d93f1f1b171dc2a27575` | `test_fault`/`test_stateful`/`test_stateful_single` を含む descriptor scan Pass |
| CLAP | `ruviform_plugin_test_clap-61ee98ac8bce960a.dll` | `3330419055b034d5118d0a2139c223a5e63bfff17063458d6bd877aa9e177742` | `test_fault`/`test_stateful` を含む descriptor scan Pass |

artifact解決は explicit path override → un-hashed artifact → profile-root
artifact → hashed dep-artifact globの順。implicit candidateは sibling dep-infoが
`Cargo.lock` の ruviform resolved revに対応する cargo git checkout sourceを
参照するものだけを採用し、stale/foreign/unprovenanced artifactは fail-closedに
拒否する。今回は provenanceを確認した hashed dep artifactが使われた。pathだけ
ではなく SHA-256を記録し、artifactが存在しない場合にskipで成功扱いしない。

Machine rows: `vst3-artifact.jsonl`、`clap-artifact.jsonl`、`vst3-scan.jsonl`、
`clap-scan.jsonl`。

## Typed runtime endpoint capability — measured Unsupported

Coordinator決裁条件(a)どおり、production `ProcessorConnection`/typed graphが
要求する endpoint capabilityは `PluginHost::prepare_endpoint` で実測した。
VST3/CLAPとも次の実測値で `Unsupported` だった。

| format | verdict | observed error kind | location | factory capability | root cause |
| --- | --- | --- | --- | --- | --- |
| VST3 `test_stateful` | Unsupported | `EndpointUnavailable` | `PluginHost::prepare_endpoint` | `create_sendable=false` | factory `create_sendable()` is `None` for `!Send` real ABI plugin wrappers |
| CLAP `test_stateful` | Unsupported | `EndpointUnavailable` | `PluginHost::prepare_endpoint` | `create_sendable=false` | factory `create_sendable()` is `None` for `!Send` real ABI plugin wrappers |

Machine rows: `vst3-typed-runtime-endpoint.jsonl`、
`clap-typed-runtime-endpoint.jsonl`。これは実在する ruviform側の capability
gapであり、typed graph PCM attach成功の証拠ではない。本 sliceの PCM oracleは
Coordinator決裁(A)/(b)どおり、同じ production `PluginHost` cellの非 RT
`PluginHost::process`経路で駆動した。private APIや test-only backdoorは追加
していない。

## Capture capability matrix

43 case rowsの verdict集計は **Pass 29 / Unsupported 14 / Fail 0 / NotRun 0**。
matrix部分は Format × plugin × lifecycle × CaptureModeで、実測 qualityを
要求 modeと区別して記録した。

### Stateful plugins

Processing中は stable stop boundaryを観測できず、要求 modeに関わらず
`BestEffort` capture。Stopped stable boundaryでは `Coordinated` modeのみ
`Coordinated`、`Quiescent`/`BestEffort` modeは `Quiescent`を達成した。

| format | plugin | lifecycle | mode | verdict | achieved quality | blob len |
| --- | --- | --- | --- | --- | --- | --- |
| VST3 | `test_stateful` | Processing | Coordinated | Pass | BestEffort | 48 |
| VST3 | `test_stateful` | Processing | Quiescent | Pass | BestEffort | 48 |
| VST3 | `test_stateful` | Processing | BestEffort | Pass | BestEffort | 48 |
| VST3 | `test_stateful` | Stopped | Coordinated | Pass | Coordinated | 48 |
| VST3 | `test_stateful` | Stopped | Quiescent | Pass | Quiescent | 48 |
| VST3 | `test_stateful` | Stopped | BestEffort | Pass | Quiescent | 48 |
| VST3 | `test_stateful_single` | Processing | Coordinated | Pass | BestEffort | 28 |
| VST3 | `test_stateful_single` | Processing | Quiescent | Pass | BestEffort | 28 |
| VST3 | `test_stateful_single` | Processing | BestEffort | Pass | BestEffort | 28 |
| VST3 | `test_stateful_single` | Stopped | Coordinated | Pass | Coordinated | 28 |
| VST3 | `test_stateful_single` | Stopped | Quiescent | Pass | Quiescent | 28 |
| VST3 | `test_stateful_single` | Stopped | BestEffort | Pass | Quiescent | 28 |
| CLAP | `test_stateful` | Processing | Coordinated | Pass | BestEffort | 20 |
| CLAP | `test_stateful` | Processing | Quiescent | Pass | BestEffort | 20 |
| CLAP | `test_stateful` | Processing | BestEffort | Pass | BestEffort | 20 |
| CLAP | `test_stateful` | Stopped | Coordinated | Pass | Coordinated | 20 |
| CLAP | `test_stateful` | Stopped | Quiescent | Pass | Quiescent | 20 |
| CLAP | `test_stateful` | Stopped | BestEffort | Pass | Quiescent | 20 |

### Stateless `test_fault`

`test_fault`には state extensionがなく、VST3/CLAPとも Processing/Stopped ×
`Coordinated`/`Quiescent`/`BestEffort` の全 12 rowsが `Unsupported`。
これらは失敗ではなく capabilityの正確な観測値として記録した。fault band
（abort/exit/hang）は in-process runnerを殺す L1-only faultなので本 slice
では armしていない。

## RFST / blob shape independent oracle

test内の独立 parserが magic・version・length・FNV-1a-32 checksum・gain値を
検査した。adapterや fixtureの parse実装は oracleに再利用していない。

| target | captured blob shape | initial state oracle |
| --- | --- | --- |
| VST3 `test_stateful` | 48 bytes = `[u32 comp_len][20-byte component RFST][u32 ctrl_len][20-byte controller RFST]` | component plain `1.0`、controller normalized `0.5` |
| VST3 `test_stateful_single` | 28 bytes = `[u32 comp_len][20-byte component RFST][u32 ctrl_len=0]` | component plain `1.0` |
| CLAP `test_stateful` | raw 20-byte RFST stream | plain `1.0` |

VST3 distributed controller payloadは processor-side parameter eventでは更新
されず normalized `0.5`のままだった。この事実を mutation oracleに混同せず、
component plain gainを state mutation oracleに使った。

## Save → mutate → restore → save round-trip

全対象で normalized `0.375`（plain `0.75`）の parameter eventを投入し、
`out = in * gain`の deterministic PCM oracleを照合した。固定入力は left
`0.5` / right `-0.25`、frames `64`。

| case | verdict | initial blob SHA-256 | mutated blob SHA-256 | restored blob SHA-256 | PCM |
| --- | --- | --- | --- | --- | --- |
| VST3 `test_stateful` | Pass | `65a76e693fc229758977b539f355e9974b7bdaeda236aaf4e470f069fb91006e` | `75be4d71fca1fc6eaeacd3dc80c792b05d29f9854bc1f3cc788c56088af908ec` | `65a76e693fc229758977b539f355e9974b7bdaeda236aaf4e470f069fb91006e` | initial `0.5/-0.25` → mutated `0.375/-0.1875` → restored `0.5/-0.25` |
| VST3 `test_stateful_single` | Pass | `8034c89555c4684142807f7d1b760e61de9bc03fe9cc3dceb185a41a8767eb3e` | `5074c8ac4ab6760e5cfc65b91be6176ad4be05b80724f498157c4d8214b899b5` | `8034c89555c4684142807f7d1b760e61de9bc03fe9cc3dceb185a41a8767eb3e` | initial `0.5/-0.25` → mutated `0.375/-0.1875` → restored `0.5/-0.25` |
| CLAP `test_stateful` | Pass | `5951a387e30e669a21ba5e8078ac8d79c14f22593513d7702fb68383fae1a022` | `f47b0eceeb1825031c11f181432e762b69f36e277f56e167739370852ff5f635` | `5951a387e30e669a21ba5e8078ac8d79c14f22593513d7702fb68383fae1a022` | initial `0.5/-0.25` → mutated `0.375/-0.1875` → restored `0.5/-0.25` |

各 caseで receiptの `blob_digest` は独立計算した blob SHA-256と一致し、
restore後の再 save blobは initial blobと byte/hash一致した。restore outcomeは
`Restored`。Machine rows: `vst3-test_stateful-roundtrip.jsonl`、
`vst3-test_stateful_single-roundtrip.jsonl`、`clap-test_stateful-roundtrip.jsonl`。

## Content-addressed blob integrity

real ABI capture由来 blobを `DirectoryPluginStateStore` に保存し、1 byte
改竄と削除を行った。

| format | corrupt blob | missing blob |
| --- | --- | --- |
| VST3 `test_stateful` | Pass — `Failed { kind: Corrupt }` | Pass — `Failed { kind: Missing }` |
| CLAP `test_stateful` | Pass — `Failed { kind: Corrupt }` | Pass — `Failed { kind: Missing }` |

どちらも store側 digest/existence precheckで typed failureになり、壊れた/
存在しない blobを黙って pluginへ restoreしなかった。Machine rows:
`*-integrity-corrupt.jsonl`、`*-integrity-missing.jsonl`。

## 実行コマンドと結果

証拠生成を兼ねた consumer-run:

```sh
SYNVELIS_TIM04_REAL_ABI_EVIDENCE_DIR=evidence/sv128 mise run test synvelis-player --test tim04_real_abi_state
```

結果: exit code 0、6/6 tests Pass。

| test | result |
| --- | --- |
| `artifact_resolution_and_scan_surface_fail_closed` | Pass |
| `capability_matrix_is_measured_for_real_test_plugins` | Pass |
| `vst3_distributed_stateful_roundtrip_oracle` | Pass |
| `vst3_single_stateful_roundtrip_oracle` | Pass |
| `clap_stateful_roundtrip_oracle` | Pass |
| `corrupt_and_missing_blobs_restore_as_typed_failures` | Pass |

必須検証laneの結果は `verification.md` に記録する。実行時の raw console
logは local worktreeの `verification-local.log` に残るが、repository
`.gitignore` の `*.log` 規則により tracked evidenceには含めない。

## Evidence scope / non-claims

- Passは Windows x86_64・in-process/L0・pinned ruviform revの Test Pluginに
  対する consumer-run。市販 plugin、全 OS、全 host mode、3OS横断の受入では
  ない。
- PCM oracleは manual production `PluginHost::process`経路。typed graph/
  `ProcessorConnection` attachは `create_sendable=None`起因の
  `EndpointUnavailable`で `Unsupported`であり、成功を主張しない。
- Processing中 `BestEffort` captureを exact/BitExactとして扱わない。
  `Coordinated`/`Quiescent`は stopped stable boundaryでの実測値である。
- `test_fault`の state capture `Unsupported`は capability観測であり、
  fault injectionの実行ではない。abort/exit/hangは NotRun。
- Offline render実組込み、VCS snapshot統合、製品 registry/discovery、
  GUI/Native Editor、AU、物理機材、AudioLens cross-gateは本 sliceの対象外。

## 発見 — ruviform typed runtime capability gap

実 ABI instanceを production typed runtimeに接続しようとすると、VST3/CLAP
両 formatで factory `create_sendable()`が `None`（in-process wrapperが
thread-affine/`!Send`）なため `PluginHost::prepare_endpoint`が
`HostError::EndpointUnavailable`を返す。state snapshot自体は manual
production host lifecycleで検証できたが、typed runtime PCM endpointは現行
ruviform revでは接続不可。ruviform側 follow-up Issue起票の判断材料として
Coordinatorへ報告する。
