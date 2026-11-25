# Issue #115 RCD-01 Virtual Endpoint EXT-L3 Harness — Verification

Date: 2026-11-25  
Branch: `feat/115-rcd01-virtual-endpoint`（base `af159f30eef33a74220a2c45614deb7d4e6278d5` + working tree changes）  
OS/arch: Windows / x86_64  
Toolchain: rustc 1.99.1 (58cf8f9c3 2026-10-13), cargo 1.99.1  
Resolved ruviform rev: `20dc41f71c47674b3e51d1eb0921124b94f8bfaf`

本ファイルは consumer virtual-WASAPI EXT-L3 harness slice の実行記録。
canonical 検証は mise lane。cargo 直接実行は補助証拠として扱う。

## Provisioning / endpoint observation

Command:

```sh
mise run test-recording-virtual-endpoint
```

Exit code: `0`

実出力（抜粋）:

```text
host: WASAPI
render candidate scan: "LG FULL HD (NVIDIA High Definition Audio)" stem=None ...
render candidate scan: "Analogue 1/2 (3- Audient iD4)" stem=None ...
render candidate scan: "Digital Audio (S/PDIF) (High Definition Audio Device)" stem=None ...
render candidate scan: "Loop-back 1/2 (3- Audient iD4)" stem=None ...
capture candidate scan: "Analogue 1/2 (3- Audient iD4)" stem=None ...
capture candidate scan: "Loop-back 1/2 (3- Audient iD4)" stem=None ...
verdict: NotRun — NOT_PROVISIONED (no Ruviform Cable Input/Output endpoint pair; other virtual drivers are not provisioning evidence)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

terminal RunMetadata JSON（verbatim）:

```json
{"case_id":"sv115.rcd01.virtual-endpoint.render-capture-recorder","requirement":"required","result":"not-run","level":"ext-l3","environment_kind":"virtual-os-endpoint","oracle":"marker-rms-journal-commit-reload-v1","parameters":{"seed":277,"fixture_version":1,"clock_config":{"sample_rate":48000,"epoch":0,"offset_ticks":0,"drift_ppm":0,"jitter":{"seed":0,"period_frames":0,"amplitude_ticks":0}}},"repo":"scarecrow173/synvelis","sha":"af159f30eef33a74220a2c45614deb7d4e6278d5","os":"windows","arch":"x86_64","expected":"","observed":"NOT_PROVISIONED — no Ruviform render↔capture virtual endpoint stem pair","evidence":[]}
```

判定: 本実行環境には `Ruviform Cable Input` / `Ruviform Cable Output` /
`Ruviform Virtual Audio Device` stem pair が列挙されなかった。したがって
consumer EXT-L3 round-trip は **NotRun(NOT_PROVISIONED)** であり、Pass ではない。
harness は fail-closed に NotRun metadata を出し、lane exit code は 0 だった。

重要な区別: upstream ruviform#1001 / PR #1008（merge `f332e462`）では
SysVAD 派生 `Ruviform Cable Input/Output` の install→実 WASAPI render→capture
round-trip が GitHub-hosted Windows で Pass 済みと報告されている。本記録の
NotRun は upstream base capability の否定ではなく、**この開発 host に endpoint
が provisioning されていないこと**と、Synvelis consumer harness の実走未達を
意味する。

## Harness implementation evidence

| 項目 | 実装 | 状態 |
| --- | --- | --- |
| endpoint pair 判定 | `ruviform` + `cable` または `virtual audio device` を含む名前だけを stem 化し、direction token を除いた render/capture pair を候補化。VB-CABLE/Voicemeeter/物理名は reject | 実装済み・pure test Pass |
| deterministic stimulus | `PcmFixture`/`PcmPattern::Impulse`（0.8 amplitude、100ms period）を output callback で absolute render-frame cursor から fill | 実装済み・endpoint 不在のため実走 NotRun |
| production Recorder capture | production negotiation/config、`InputSession`/`InputAdapter`/`RealDeviceRig`/実 `StreamWriter`/journal/staging/commit/save/reload を通る経路 | 実装済み・endpoint 不在のため実走 NotRun |
| multi-track | stereo 以上 capture を `ChannelMap::mono(0)`/`mono(1)` の 2 track/2 lane/2 stream に分離 | 実装済み・endpoint 不在のため実走 NotRun |
| oracle | marker 個数/間隔/latency/RMS/非 finite sample + journal verified prefix/CRC/`verified_sha256` 再計算 + commit/reload digest chain | 実装済み・endpoint 不在のため実走 NotRun |
| fail-closed | host 列挙失敗/pair 不在は NotRun、候補 pair 失敗は Fail | 実装済み・pair 不在 NotRun を実測 |

## Canonical verification lanes

| Lane | Exit code | Result | Note |
| --- | ---: | --- | --- |
| `mise run fmt-check` | 1 | Fail（environment/tooling） | Windows 上で `cargo fmt --all -- --check` が `ファイル名または拡張子が長すぎます。 (os error 206)` で rustfmt 起動に失敗。コード formatting 差分ではなく workspace-wide rustfmt invocation の起動失敗（`cargo fmt -v` では多数 target を列挙後に失敗）。補助証拠として `cargo fmt -p synvelis-recording -- --check` は exit 0 |
| `mise run check synvelis-recording` | 0 | Pass | `virtual_endpoint` compile を含む |
| `mise run lint synvelis-recording` | 0 | Pass | 既定 feature/all targets |
| `mise run test synvelis-recording` | 0 | Pass | pure tests 4/4 を含む全既定 tests Pass |
| `mise run lint-recording-real-input` | 0 | Pass | `--features real-device --all-targets` に新 test target も含まれる |
| `mise run test-recording-virtual-endpoint` | 0 | Pass（lane）/ EXT-L3 NotRun | endpoint 未 provisioning のため `result=not-run`/`NOT_PROVISIONED` を出力して exit 0。Pass 偽装なし |
| `mise exec -- prek run --all-files` | 0 | Pass | trailing whitespace/end-of-file/YAML/TOML/mixed-line-ending/actionlint hooks Pass |

## Honest verdict

- Synvelis consumer virtual endpoint harness は実装され、compile/lint/pure tests
  と fail-closed NotRun 動作は検証済み。
- 本環境で Ruviform virtual endpoint pair は provisioning されていないため、
  EXT-L3 render→capture→Recorder round-trip は **NotRun**。
- この NotRun を upstream ruviform#1001 の base round-trip Pass と混同しない。
  また、fake port/synthetic-L1/rf628 deterministic-backend/physical real-input
  lane の Pass を EXT-L3 Pass へ転用しない。
