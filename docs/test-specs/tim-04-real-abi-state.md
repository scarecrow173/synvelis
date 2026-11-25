---
id: "TSPEC-034Xn0hBopO82wSvv0Y0Yn"
type: "test-spec"
status: "approved"
title: "TIM-04 Plugin State Snapshot — real ABI consumer-run test spec"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/tim-04-real-abi-state.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-04-plugin-state-snapshot.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-04-real-abi-state.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-04-real-abi-state.md"
  verified-by: []
  references:
    - "docs/designs/tim-04-real-abi-state.md"
    - "evidence/sv128/real-abi-state.md"
  defers: []
  deferred-by: []
---

# Test Spec — TIM-04 Plugin State Snapshot real ABI consumer-run

## Purpose

Issue #128 の実 ABI consumer-run sliceを、mock/fake pluginに依存しない
VST3/CLAP Test Plugin実バイナリで検証する。証拠は
`crates/synvelis-player/tests/tim04_real_abi_state.rs` と
`evidence/sv128/` に残す。本 test specの Passは Windows x86_64・
in-process/L0・ruviform pinned rev `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`
に対する consumer-runであり、Issue #128全体の統合受入ではない。

## Feature

正規 ABI Test Pluginを production `PluginHost` cellに登録し、state
capture/save/restoreを `PluginStateHost`・`PluginStateSession`・
`DirectoryPluginStateStore` で実走する。RFST state bytes・content digest・
deterministic PCMを独立oracleとして照合し、capture qualityと unsupported
capabilityを実測値どおり記録する。

## Rules

- R1: artifactは fail-closedに解決する。見つからない場合に testを skip
  して成功扱いしない。
- R2: host lifecycleは production `PluginHost` cell上で
  Scan → Instantiate → Initialize → Activate → StartProcessing →
  StopProcessing → Deactivate → Destroy を辿る。private APIや test-only
  backdoorを使わない。
- R3: typed endpoint capabilityは実測する。現行 revでは
  `create_sendable() = None` のため `prepare_endpoint` が
  `EndpointUnavailable` を返し、typed graph PCM attachは `Unsupported`。
- R4: Processing中 captureは `BestEffort`。stable stop boundaryでは
  `Coordinated`/`Quiescent` を実測する。stateless `test_fault` は
  `Unsupported`。
- R5: RFST parserは magic/version/length/FNV-1a checksumと plain/normalized
  gainを検査する。VST3 distributed frame、VST3 single empty-controller
  frame、CLAP raw streamを区別する。
- R6: blob SHA-256は receiptの `blob_digest` と一致する。restore後の再
  saveは initial blobと byte/hash一致する。
- R7: normalized `0.375` の parameter eventは plain gain `0.75` に写像し、
  PCMは `out = in * gain` に従う。restore後は初期 PCMに戻る。
- R8: corrupt blobは `Failed(Corrupt)`、missing blobは `Failed(Missing)`。
  どちらも黙って plugin restoreしない。
- R9: `test_fault` の abort/exit/hangは armしない。RT callback内で artifact
  load/state save/FS IOを行わない。

## Cases

| case | driver | expected verdict | oracle |
| --- | --- | --- | --- |
| RAS-ART-VST3/CLAP | cdylib resolver + library load + Scan | Pass | path/file existence、SHA-256、required descriptors |
| RAS-CAP-MATRIX | `PluginStateHost::capture` | Pass/Unsupported | lifecycle/modeごとの achieved qualityと `test_fault` の Unsupported |
| RAS-ENDPOINT-VST3/CLAP | `PluginHost::prepare_endpoint` | Unsupported | error kind `EndpointUnavailable`、発生地点、`create_sendable=false` |
| RAS-RT-VST3-DIST | save→PCM mutation→restore→save | Pass | 48-byte frame、component plain/controller normalized、digest、PCM復元 |
| RAS-RT-VST3-SINGLE | save→PCM mutation→restore→save | Pass | 28-byte frame、plain gain、digest、PCM復元 |
| RAS-RT-CLAP | save→PCM mutation→restore→save | Pass | raw 20-byte RFST、plain gain、digest、PCM復元 |
| RAS-CORRUPT-VST3/CLAP | blob 1-byte flip + restore | Pass | `Failed(Corrupt)` |
| RAS-MISSING-VST3/CLAP | blob delete + restore | Pass | `Failed(Missing)` |

## Test entry points

`crates/synvelis-player/tests/tim04_real_abi_state.rs` の test関数:

| test | 内容 |
| --- | --- |
| `artifact_resolution_and_scan_surface_fail_closed` | VST3/CLAP artifact解決、hash、library load、Scan descriptor |
| `capability_matrix_is_measured_for_real_test_plugins` | quality/capability matrixと typed endpoint probe |
| `vst3_distributed_stateful_roundtrip_oracle` | VST3 distributed `test_stateful` の state/PCM round-trip |
| `vst3_single_stateful_roundtrip_oracle` | VST3 `test_stateful_single` の state/PCM round-trip |
| `clap_stateful_roundtrip_oracle` | CLAP `test_stateful` の state/PCM round-trip |
| `corrupt_and_missing_blobs_restore_as_typed_failures` | VST3/CLAPの corrupt/missing blob typed failure |

## Verification commands

証拠生成を兼ねる限定実行:

```sh
SYNVELIS_TIM04_REAL_ABI_EVIDENCE_DIR=evidence/sv128 mise run test synvelis-player --test tim04_real_abi_state
```

必須検証lane:

```sh
mise run fmt-check
mise run check synvelis-player
mise run lint synvelis-player
mise run test synvelis-player
mise run test-rt
mise run check-headless-deps
mise run check-ruviform-rev
```

## Measured results

2026-11-25・Windows x86_64・in-process/L0で、証拠生成コマンドは
exit code 0、6/6 tests Passだった。machine-readable evidenceは43 case行で、
verdict集計は Pass 29・Unsupported 14・Fail 0・NotRun 0。

主な実測値:

- VST3 artifact: `ruviform_plugin_test_vst3-bbc6a23ab156b55c.dll`、
  SHA-256 `51c2cf9a0a3201e838bb32acf3b4a0b97a3b24b5b0a63804e309b0872a13b591`。
- CLAP artifact: `ruviform_plugin_test_clap-61ee98ac8bce960a.dll`、
  SHA-256 `cd476a4384a24fdc1b17ce7b4ad70a722167a13a66101b53f4cb052f4c895f4a`。
- typed endpoint: VST3/CLAPとも `PluginHost::prepare_endpoint` が
  `EndpointUnavailable`、factory capability `create_sendable=false`。
  verdictは capabilityどおり `Unsupported`。
- stateful capture matrix: Processing中は全 modeで `BestEffort`。
  stopped stable boundaryでは `Coordinated` modeが `Coordinated`、
  `Quiescent`/`BestEffort` modeが `Quiescent`。
- `test_fault`: VST3/CLAP・全 lifecycle/modeで `Unsupported`。
- VST3 distributed round-trip: blob 48 bytes。initial plain `1.0`/
  controller normalized `0.5`、mutated component plain `0.75`/
  controller normalized `0.5`、restored blobは initialと byte/hash一致。
- VST3 single round-trip: blob 28 bytes。plain `1.0`→`0.75`→`1.0`、
  restored blobは initialと byte/hash一致。
- CLAP round-trip: raw blob 20 bytes。plain `1.0`→`0.75`→`1.0`、
  restored blobは initialと byte/hash一致。
- PCM: 初期 left `0.5`/right `-0.25`、mutation後 left `0.375`/right
  `-0.1875`、restore後 left `0.5`/right `-0.25`。
- corrupt/missing: VST3/CLAPとも `Failed(Corrupt)`・`Failed(Missing)`。

## Guarantees

- mock/fake pluginを使わず、実 VST3/CLAPバイナリの state surfaceを通す。
- `BestEffort` captureを exact/BitExactとして扱わない。
- `Unsupported` capabilityを失敗へ、または成功へ無断で翻訳しない。
- blob bytesを adapterの返り値だけで信用せず、RFST parser・SHA-256・PCMで
  独立に照合する。
- typed endpointが通らない事実を隠さず、manual host pathの成功を typed
  graph attach成功と混同しない。

## Non-goals / NotRun

- L1 crash/hang/timeout isolation、process supervisor、専用 runner。
- `test_fault` abort/exit/hang bandの発火。
- Offline render実組込み、VCS snapshot統合、製品 registry/discovery。
- 実 GUI/Native Editor、AU、市販 plugin、物理機材、AudioLens cross-gate、
  3OS横断。

## Risks

- VST3 distributed controller payloadは processor-side eventで更新されず、
  normalized `0.5` のまま残る。これを誤って mutation oracleに使わないよう、
  component plainとcontroller normalizedを分けて記録する。
- hashed cdylib artifact名は cargo hashで変わる。evidenceでは file nameと
  SHA-256を併記し、pathだけを再現性識別子にしない。
